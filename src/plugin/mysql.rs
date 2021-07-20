use std::sync::Arc;

use appbase::*;
use mysql::*;
use mysql::prelude::*;

pub struct MySqlPlugin {
    base: PluginBase,
}

appbase_plugin_requires!(MySqlPlugin; );

impl Plugin for MySqlPlugin {
    appbase_plugin_default!(MySqlPlugin);

    fn new() -> Self {
        MySqlPlugin {
            base: PluginBase::new(),
        }
    }

    fn initialize(&mut self) {
        if !self.plugin_initialize() {
            return;
        }
    }

    fn startup(&mut self) {
        if !self.plugin_startup() {
            return;
        }
    }

    fn shutdown(&mut self) {
        if !self.plugin_shutdown() {
            return;
        }
    }
}
