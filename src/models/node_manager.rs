use super::Node;

use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct NodeManager {
    nodes: Arc<Mutex<Vec<Node>>>,
}

impl NodeManager {
    pub fn new() -> Self {
        let nodes = vec![
            Node {
                name: String::from("node 1"),
            },
            Node {
                name: String::from("node 2"),
            },
            Node {
                name: String::from("node 3"),
            },
        ];

        Self {
            nodes: Arc::new(Mutex::new(nodes)),
        }
    }

    pub async fn list_nodes(&mut self) -> Vec<Node> {
        let getter = self.nodes.lock().await;
        getter.clone().into_iter().collect()
    }
}
