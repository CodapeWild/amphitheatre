// Copyright 2023 The Amphitheatre Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use amp_common::config::CredentialConfiguration;
use k8s_openapi::api::core::v1::ObjectReference;
use kube::runtime::events::Recorder;
use kube::Client;
use tokio::sync::RwLock;

use crate::config::Config;

/// The core type through which handler functions can access common API state.
///
/// This can be accessed by adding a parameter `Extension<Context>` to a handler
///  function's  parameters.
///
/// It may not be a bad idea if you need your API to be more modular (turn routes
/// on and off, and disable any unused extension objects) but it's really up to a
/// judgement call.
pub struct Context {
    pub k8s: Client,
    pub configuration: RwLock<CredentialConfiguration>,
    pub config: Config,
}

impl Context {
    pub async fn new(config: Config) -> anyhow::Result<Context> {
        Ok(Context {
            k8s: Client::try_default().await?,
            configuration: RwLock::new(CredentialConfiguration::default()),
            config,
        })
    }

    pub fn recorder(&self, reference: ObjectReference) -> Recorder {
        Recorder::new(self.k8s.clone(), "amp-controllers".into(), reference)
    }
}
