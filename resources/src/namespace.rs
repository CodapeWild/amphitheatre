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

use std::collections::BTreeMap;

use amp_common::schema::Playbook;
use k8s_openapi::api::core::v1::Namespace;
use kube::api::{Patch, PatchParams};
use kube::core::ObjectMeta;
use kube::{Api, Client, Resource, ResourceExt};

use super::error::{Error, Result};

pub async fn create(client: &Client, playbook: &Playbook) -> Result<Namespace> {
    let api: Api<Namespace> = Api::all(client.clone());

    let name = playbook.spec.namespace.clone();
    let owner_reference = playbook.controller_owner_ref(&()).unwrap();
    let resource = Namespace {
        metadata: ObjectMeta {
            name: Some(name.clone()),
            owner_references: Some(vec![owner_reference]),
            labels: Some(BTreeMap::from([
                ("app.kubernetes.io/managed-by".into(), "Amphitheatre".into()),
                ("syncer.amphitheatre.app/sync".into(), "true".into()),
            ])),
            ..ObjectMeta::default()
        },
        ..Namespace::default()
    };
    tracing::debug!("The namespace resource:\n {:?}\n", resource);

    let namespace = api
        .patch(
            &name,
            &PatchParams::apply("amp-controllers").force(),
            &Patch::Apply(&resource),
        )
        .await
        .map_err(Error::KubeError)?;

    tracing::info!("Added namespace: {}", namespace.name_any());
    Ok(namespace)
}
