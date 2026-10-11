//! Read a .glb (binary glTF): node hierarchy, skin joints, animation clips and the
//! root-motion curves our packer stores in animation extras. Float data only.

use crate::json::Json;
use crate::qmath::{Quat, Trs, V3};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    pub parent: Option<usize>,
    pub bind: Trs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Path {
    Rotation,
    Translation,
}

#[derive(Clone, Debug)]
pub struct Track {
    pub node: usize,
    pub path: Path,
    pub times: Vec<f32>,
    /// 4 floats per key for rotation, 3 for translation
    pub values: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct Clip {
    pub name: String,
    /// last key time (as three.js computes it)
    pub duration: f32,
    pub tracks: Vec<Track>,
    /// per frame [forward, up] uu (packer extras, e.g. AR_Mantle_Start)
    pub root_curve: Option<Vec<[f32; 2]>>,
}

#[derive(Clone, Debug, Default)]
pub struct Model {
    pub nodes: Vec<Node>,
    /// nodes in an order where parents come before children
    pub order: Vec<usize>,
    pub by_name: HashMap<String, usize>,
    pub clips: Vec<Clip>,
    pub clip_by_name: HashMap<String, usize>,
}

impl Model {
    pub fn node(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied()
    }
    pub fn clip(&self, name: &str) -> Option<&Clip> {
        self.clip_by_name.get(name).map(|&i| &self.clips[i])
    }
    pub fn bind_pose(&self) -> Vec<Trs> {
        self.nodes.iter().map(|n| n.bind).collect()
    }
    /// Model-space transforms of every node for local pose `local`.
    pub fn model_space(&self, local: &[Trs]) -> Vec<Trs> {
        let mut out = vec![Trs::default(); local.len()];
        for &i in &self.order {
            out[i] = match self.nodes[i].parent {
                Some(p) => out[p].compose(local[i]),
                None => local[i],
            };
        }
        out
    }

    pub fn parse(bytes: &[u8]) -> Result<Model, String> {
        if bytes.len() < 20 || &bytes[0..4] != b"glTF" {
            return Err("not a .glb".into());
        }
        let u32le = |o: usize| {
            u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]) as usize
        };
        let jlen = u32le(12);
        let json = std::str::from_utf8(&bytes[20..20 + jlen]).map_err(|e| e.to_string())?;
        let g = Json::parse(json)?;
        let bin_at = 20 + jlen;
        let bin: &[u8] = if bytes.len() >= bin_at + 8 {
            &bytes[bin_at + 8..bin_at + 8 + u32le(bin_at)]
        } else {
            &[]
        };

        let views = g
            .get("bufferViews")
            .map(|v| v.arr().to_vec())
            .unwrap_or_default();
        let accs = g
            .get("accessors")
            .map(|v| v.arr().to_vec())
            .unwrap_or_default();
        let read = |ai: usize| -> Vec<f32> {
            let a = &accs[ai];
            let n = a.get("count").and_then(Json::usize).unwrap_or(0);
            let comps = match a.get("type").and_then(Json::str).unwrap_or("SCALAR") {
                "VEC2" => 2,
                "VEC3" => 3,
                "VEC4" => 4,
                "MAT4" => 16,
                _ => 1,
            };
            let v = &views[a.get("bufferView").and_then(Json::usize).unwrap_or(0)];
            let off = v.get("byteOffset").and_then(Json::usize).unwrap_or(0)
                + a.get("byteOffset").and_then(Json::usize).unwrap_or(0);
            let stride = v
                .get("byteStride")
                .and_then(Json::usize)
                .unwrap_or(comps * 4);
            let mut out = Vec::with_capacity(n * comps);
            for i in 0..n {
                for c in 0..comps {
                    let o = off + i * stride + c * 4;
                    out.push(f32::from_le_bytes([
                        bin[o],
                        bin[o + 1],
                        bin[o + 2],
                        bin[o + 3],
                    ]));
                }
            }
            out
        };

        let jn = g.get("nodes").map(|v| v.arr().to_vec()).unwrap_or_default();
        let mut nodes: Vec<Node> = jn
            .iter()
            .map(|n| {
                let f = |k: &str, d: &[f32]| -> Vec<f32> {
                    n.get(k)
                        .map(|a| a.arr().iter().map(Json::f32).collect())
                        .unwrap_or_else(|| d.to_vec())
                };
                let t = f("translation", &[0.0, 0.0, 0.0]);
                let r = f("rotation", &[0.0, 0.0, 0.0, 1.0]);
                let s = f("scale", &[1.0, 1.0, 1.0]);
                Node {
                    name: n.get("name").and_then(Json::str).unwrap_or("").to_string(),
                    parent: None,
                    bind: Trs {
                        t: V3::new(t[0], t[1], t[2]),
                        r: Quat::new(r[0], r[1], r[2], r[3]),
                        s: V3::new(s[0], s[1], s[2]),
                    },
                }
            })
            .collect();
        for (i, n) in jn.iter().enumerate() {
            for c in n
                .get("children")
                .map(|c| c.arr().to_vec())
                .unwrap_or_default()
            {
                if let Some(ci) = c.usize() {
                    nodes[ci].parent = Some(i);
                }
            }
        }
        // parents before children
        let mut order = Vec::with_capacity(nodes.len());
        let mut done = vec![false; nodes.len()];
        fn visit(i: usize, nodes: &[Node], done: &mut [bool], order: &mut Vec<usize>) {
            if done[i] {
                return;
            }
            if let Some(p) = nodes[i].parent {
                visit(p, nodes, done, order);
            }
            done[i] = true;
            order.push(i);
        }
        for i in 0..nodes.len() {
            visit(i, &nodes, &mut done, &mut order);
        }
        let by_name = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.name.clone(), i))
            .collect();

        let mut clips = Vec::new();
        for a in g
            .get("animations")
            .map(|v| v.arr().to_vec())
            .unwrap_or_default()
        {
            let samplers = a
                .get("samplers")
                .map(|v| v.arr().to_vec())
                .unwrap_or_default();
            let mut tracks = Vec::new();
            let mut duration = 0.0f32;
            for ch in a
                .get("channels")
                .map(|v| v.arr().to_vec())
                .unwrap_or_default()
            {
                let tgt = ch.get("target").unwrap_or(&Json::Null);
                let path = match tgt.get("path").and_then(Json::str) {
                    Some("rotation") => Path::Rotation,
                    Some("translation") => Path::Translation,
                    _ => continue,
                };
                let (Some(node), Some(si)) = (
                    tgt.get("node").and_then(Json::usize),
                    ch.get("sampler").and_then(Json::usize),
                ) else {
                    continue;
                };
                let s = &samplers[si];
                let times = read(s.get("input").and_then(Json::usize).unwrap_or(0));
                let values = read(s.get("output").and_then(Json::usize).unwrap_or(0));
                if let Some(&t) = times.last() {
                    duration = duration.max(t);
                }
                tracks.push(Track {
                    node,
                    path,
                    times,
                    values,
                });
            }
            let root_curve = a.get("extras").and_then(|e| e.get("root")).map(|r| {
                r.arr()
                    .iter()
                    .map(|p| {
                        [
                            p.idx(0).map(Json::f32).unwrap_or(0.0),
                            p.idx(1).map(Json::f32).unwrap_or(0.0),
                        ]
                    })
                    .collect()
            });
            clips.push(Clip {
                name: a.get("name").and_then(Json::str).unwrap_or("").to_string(),
                duration,
                tracks,
                root_curve,
            });
        }
        let clip_by_name = clips
            .iter()
            .enumerate()
            .map(|(i, c)| (c.name.clone(), i))
            .collect();
        Ok(Model {
            nodes,
            order,
            by_name,
            clips,
            clip_by_name,
        })
    }
}

impl Track {
    fn key(&self, t: f32) -> (usize, usize, f32) {
        let n = self.times.len();
        if n <= 1 || t <= self.times[0] {
            return (0, 0, 0.0);
        }
        if t >= self.times[n - 1] {
            return (n - 1, n - 1, 0.0);
        }
        let i = self.times.partition_point(|&x| x <= t) - 1;
        let (a, b) = (self.times[i], self.times[i + 1]);
        (i, i + 1, if b > a { (t - a) / (b - a) } else { 0.0 })
    }
    pub fn rot(&self, t: f32) -> Quat {
        let (i, j, k) = self.key(t);
        let q = |i: usize| {
            Quat::new(
                self.values[i * 4],
                self.values[i * 4 + 1],
                self.values[i * 4 + 2],
                self.values[i * 4 + 3],
            )
        };
        q(i).slerp(q(j), k)
    }
    pub fn pos(&self, t: f32) -> V3 {
        let (i, j, k) = self.key(t);
        let v = |i: usize| {
            V3::new(
                self.values[i * 3],
                self.values[i * 3 + 1],
                self.values[i * 3 + 2],
            )
        };
        v(i).lerp(v(j), k)
    }
}
