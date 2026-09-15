pub mod builtin;
pub mod policy;
pub mod prompt;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::llm::ToolSpec;

#[async_trait]
pub trait Tool: Send + Sync {
    fn spec(&self) -> ToolSpec;
    fn subject(&self, args: &Value) -> String;
    async fn run(&self, args: &Value, cwd: &Path) -> Result<String, String>;
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.spec().name.clone(), tool);
    }

    pub fn specs(&self) -> Vec<ToolSpec> {
        self.tools.values().map(|tool| tool.spec()).collect()
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }
}

pub fn builtin_registry(roots: builtin::Roots) -> ToolRegistry {
    let mut registry = ToolRegistry::default();
    registry.register(Arc::new(builtin::ReadFile {
        roots: roots.clone(),
    }));
    registry.register(Arc::new(builtin::EditFile {
        roots: roots.clone(),
    }));
    registry.register(Arc::new(builtin::WriteFile {
        roots: roots.clone(),
    }));
    registry.register(Arc::new(builtin::ListDir {
        roots: roots.clone(),
    }));
    registry.register(Arc::new(builtin::Grep { roots }));
    registry.register(Arc::new(builtin::RunCommand));
    registry
}
