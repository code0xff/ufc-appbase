use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use appbase::*;
use futures::lock::Mutex as FutureMutex;
use jsonrpc_core::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};


/*
 * Plugin typename MUST be unique.
 */

use crate::plugin::jsonrpc::JsonRpcPlugin;
use crate::plugin::rocks::{RocksMethod, RocksMsg, RocksPlugin};
use crate::types::block::{BlockTask, SubscribeBlock, SubscribeStatus};
use crate::validation::{subscribe, unsubscribe};


pub struct TemplatePlugin {
    /*
     * Plugin SHOULD include `base: PluginBase` as its field.
     */
    base: PluginBase,
}

/*
 * Plugin SHOULD have `appbase_plugin_requires!` macro including dependencies.
 * (case 1) Plugin A without any dependencies: `appbase_plugin_requires!(A; );`
 * (case 2) Plugin A depends on Plugin B and C: `appbase_plugin_requires!(A; B, C);`
 */
appbase_plugin_requires!(TemplatePlugin; );

/*
 * Plugin impl MAY have plugin-specific methods.
 */
impl TemplatePlugin {}

/*
 * Plugin MUST implement `Plugin` trait.
 */
impl Plugin for TemplatePlugin {
    /*
     * Plugin trait impl SHOULD have `appbase_plugin_default!` macro
     */
    appbase_plugin_default!(TemplatePlugin);

    /*
     * Plugin trait impl MUST implement following methods:
     *    fn new() -> Self;
     *    fn typename() -> String;         // automatically added by appbase_plugin_default!
     *    fn name(&self) -> String;        // automatically added by appbase_plugin_default!
     *    fn initialize(&mut self);
     *    fn startup(&mut self);
     *    fn shutdown(&mut self);
     *    fn state(&self) -> PluginState;  // automatically added by appbase_plugin_default!
     */

    fn new() -> Self {
        TemplatePlugin {
            base: PluginBase::new(),
            // ... other fields, if exist.
        }
    }

    fn initialize(&mut self) {
        /*
         * `initialize` SHOULD call `plugin_initialize` (automatically added by appbase_plugin_requires!)
         * in the first part and return at once if it returns `false`.
         * It is guaranteed that all dependencies are initialized by calling `plugin_initialize`.
         * Be careful not to make circular dependency.
         */
        if !self.plugin_initialize() {
            return;
        }

        // ... do remaining steps for initialization
    }

    fn startup(&mut self) {
        /*
         * `startup` SHOULD call `plugin_startup` in the first part.
         */
        if !self.plugin_startup() {
            return;
        }

        // ... do remaining steps for startup
    }

    fn shutdown(&mut self) {
        /*
         * `shutdown` SHOULD call `plugin_shutdown` in the first part.
         */
        if !self.plugin_shutdown() {
            return;
        }

        // ... do remaining steps for shutdown
    }
}
