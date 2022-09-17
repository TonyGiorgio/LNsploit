use super::Node;

use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct NodeManager {
    nodes: Arc<Mutex<Vec<Node>>>,
}

impl NodeManager {
    pub async fn new() -> Self {
        let mut node_manager = Self {
            nodes: Arc::new(Mutex::new(vec![])),
        };

        // TODO remove, temporary
        node_manager.new_node(String::from("node 1")).await;
        node_manager.new_node(String::from("node 2")).await;
        node_manager.new_node(String::from("node 3")).await;

        node_manager
    }

    pub async fn list_nodes(&mut self) -> Vec<Node> {
        self.nodes.lock().await.clone().into_iter().collect()
    }

    pub async fn new_node(&mut self, name: String) {
        self.nodes.lock().await.push(Node::new(name));
    }
}
