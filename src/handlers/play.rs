// Copyright 2022 The Amphitheatre Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::{database::Database, models::play::Play, services::play::PlayService};
use rocket::serde::json::Json;

#[get("/")]
pub async fn list(db: Database) -> Json<Vec<Play>> {
    let plays = PlayService::list(&db).await;
    match plays {
        Ok(plays) => Json(plays),
        Err(e) => {
            error!("{e}");
            Json(vec![])
        }
    }
}

#[get("/<id>")]
pub async fn detail(db: Database, id: u64) -> Json<Play> {
    let play = PlayService::get(&db, id).await;
    match play {
        Ok(play) => Json(play),
        Err(_) => Json(Play::default()),
    }
}
