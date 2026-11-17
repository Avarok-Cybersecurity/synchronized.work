//! Provisioning from the Worker's environment: `[vars]` and secrets, by the names `sw-node`
//! declares. A name that is neither is absent, and the node refuses to start.

use sw_node::Source;
use worker::Env;

pub struct EnvSource<'a>(pub &'a Env);

impl Source for EnvSource<'_> {
    fn get(&self, name: &str) -> Option<String> {
        self.0
            .secret(name)
            .map(|s| s.to_string())
            .or_else(|_| self.0.var(name).map(|v| v.to_string()))
            .ok()
    }
}
