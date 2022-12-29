use std::{
    collections::{BinaryHeap, HashMap},
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::{atomic::AtomicU64, Arc, Condvar, Mutex},
};

use serde::Deserialize;
use serde_json::{json, Value as JsonValue};

#[derive(Deserialize)]
#[serde(tag = "request")]
enum Request {
    #[serde(rename = "put")]
    Put {
        queue: String,
        job: JsonValue,
        pri: u64,
    },
    #[serde(rename = "get")]
    Get {
        queues: Vec<String>,
        #[serde(default)]
        wait: bool,
    },
    #[serde(rename = "delete")]
    Delete { id: u64 },
    #[serde(rename = "abort")]
    Abort { id: u64 },
}

#[derive(Clone)]
struct Job {
    id: u64,
    pri: u64,
    job: JsonValue,
}

impl PartialEq for Job {
    fn eq(&self, other: &Self) -> bool {
        self.pri == other.pri
    }
}

impl Eq for Job {}

impl PartialOrd for Job {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Job {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.pri.cmp(&other.pri)
    }
}

pub fn main() {
    let listener = TcpListener::bind("0.0.0.0:1200").unwrap();
    let queues_map: Arc<Mutex<(HashMap<String, BinaryHeap<Job>>, HashMap<u64, String>)>> =
        Default::default();
    let next_id: Arc<AtomicU64> = Default::default();
    let waker: Arc<Condvar> = Default::default();
    for incoming in listener.into_incoming() {
        let mut stream = match incoming {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("Error accepting incoming stream: {:?}", e);
                continue;
            }
        };
        let queues_map = queues_map.clone();
        let waker = waker.clone();
        let next_id = next_id.clone();
        std::thread::spawn(move || {
            let mut buffer = BufReader::new(stream.try_clone().unwrap());
            let mut processing = HashMap::new();
            loop {
                let mut bytes = Vec::new();
                let read = buffer.read_until(b'\n', &mut bytes).unwrap();
                if read == 0 {
                    stream.shutdown(std::net::Shutdown::Both).unwrap();
                    break;
                }
                let request: Request = match serde_json::from_slice(&bytes[..bytes.len() - 1]) {
                    Ok(request) => request,
                    Err(e) => {
                        eprintln!("Error parsing request: {:?}", e);
                        break;
                    }
                };

                let resp = match request {
                    Request::Put { queue, job, pri } => {
                        let mut queues = queues_map.lock().unwrap();
                        let id = next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        queues
                            .0
                            .entry(queue.clone())
                            .or_default()
                            .push(Job { id, pri, job });
                        queues.1.insert(id, queue);
                        waker.notify_all();
                        json!({
                            "status": "ok",
                            "id": id,
                        })
                    }
                    Request::Get { queues, wait } => {
                        let mut job;
                        let mut queues_map = queues_map.lock().unwrap();
                        loop {
                            job = queues
                                .iter()
                                .map(|qn| {
                                    queues_map.0.get(qn).and_then(|q| q.peek().map(|x| (x, qn)))
                                })
                                .reduce(|q1, q2| match (q1, q2) {
                                    (None, None) => None,
                                    (None, Some(b)) => Some(b),
                                    (Some(a), None) => Some(a),
                                    (Some(a), Some(b)) => Some(std::cmp::max(a, b)),
                                })
                                .flatten();
                            if let Some((j, qn)) = job {
                                if !queues_map.1.contains_key(&j.id) {
                                    queues_map.0.get_mut(qn).unwrap().pop();
                                    job = None;
                                    continue;
                                } else {
                                    break;
                                }
                            }
                            if !wait {
                                break;
                            }
                            queues_map = waker.wait(queues_map).unwrap();
                        }
                        if let Some((_, qn)) = job {
                            let job = queues_map.0.get_mut(qn).unwrap().pop().unwrap();
                            processing.insert(job.id, job.clone());
                            json!({
                                "status": "ok",
                                "id": job.id,
                                "job": job.job,
                                "pri": job.pri,
                                "queue": qn,
                            })
                        } else {
                            json!({
                                "status": "no-job",
                            })
                        }
                    }
                    Request::Delete { id } => {
                        // delete from index
                        let mut queues_map = queues_map.lock().unwrap();
                        let status = if queues_map.1.remove(&id).is_some() {
                            "ok"
                        } else {
                            "no-job"
                        };
                        json!({
                            "status": status,
                        })
                    }
                    Request::Abort { id } => {
                        // delete from processing, if not deleted (exists in index) put pack in queue
                        let status = 'val: {
                            if let Some(job) = processing.remove(&id) {
                                let mut queues_map = queues_map.lock().unwrap();
                                if let Some(qn) = queues_map.1.get(&id) {
                                    let qn = qn.clone();
                                    queues_map.0.get_mut(&qn).unwrap().push(job);
                                    waker.notify_all();
                                    break 'val "ok";
                                }
                            }
                            "no-job"
                        };
                        json!({
                            "status": status,
                        })
                    }
                };
                serde_json::to_writer(&mut stream, &resp).unwrap();
                stream.write_all(b"\n").unwrap();
            }
            let mut queues_map = queues_map.lock().unwrap();
            for (_, job) in processing {
                if let Some(qn) = queues_map.1.get(&job.id) {
                    let qn = qn.clone();
                    queues_map.0.get_mut(&qn).unwrap().push(job);
                    waker.notify_all();
                }
            }
        });
    }
}
