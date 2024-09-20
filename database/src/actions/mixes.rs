use std::collections::HashMap;

use anyhow::Context;
use anyhow::Result;
use chrono::Utc;
use log::warn;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Condition;
use sea_orm::sea_query::Expr;
use sea_orm::ActiveValue;
use sea_orm::TransactionTrait;
use sea_orm::{ColumnTrait, EntityTrait, Order, QueryFilter, QueryOrder, QuerySelect, QueryTrait};

use crate::actions::analysis::get_percentile_analysis_result;
use crate::actions::utils::generate_group_name;
use crate::connection::RecommendationDbConnection;
use crate::entities::mix_queries;
use crate::entities::mixes;
use crate::entities::{
    media_file_albums, media_file_artists, media_file_playlists, media_file_stats, media_files,
};

use super::analysis::get_centralized_analysis_result;
use super::file::get_files_by_ids;
use super::recommendation::get_recommendation_by_parameter;
use crate::get_by_id;
use crate::get_by_ids;
use crate::get_first_n;

use super::utils::CountByFirstLetter;

impl CountByFirstLetter for mixes::Entity {
    fn group_column() -> Self::Column {
        mixes::Column::Group
    }

    fn id_column() -> Self::Column {
        mixes::Column::Id
    }
}

get_by_ids!(get_mixes_by_ids, mixes);
get_by_id!(get_mixes_by_id, mixes);
get_first_n!(list_mixes, mixes);

pub async fn get_mixes_groups(
    db: &DatabaseConnection,
    groups: Vec<String>,
) -> Result<Vec<(String, Vec<mixes::Model>)>> {
    let entities: Vec<mixes::Model> = mixes::Entity::find()
        .filter(mixes::Column::Group.is_in(groups.clone()))
        .all(db)
        .await?;

    let mut grouped_entities: HashMap<String, Vec<mixes::Model>> = HashMap::new();
    for entity in entities {
        grouped_entities
            .entry(entity.group.clone())
            .or_default()
            .push(entity);
    }

    let result = groups
        .into_iter()
        .map(|group| {
            let entities_in_group = grouped_entities.remove(&group).unwrap_or_default();
            (group, entities_in_group)
        })
        .collect();

    Ok(result)
}

pub async fn create_mix(
    db: &DatabaseConnection,
    name: String,
    scriptlet_mode: bool,
    mode: i32,
    locked: bool,
) -> Result<mixes::Model> {
    use mixes::ActiveModel;

    let new_mix = ActiveModel {
        name: ActiveValue::Set(name.clone()),
        group: ActiveValue::Set(generate_group_name(&name)),
        scriptlet_mode: ActiveValue::Set(scriptlet_mode),
        mode: ActiveValue::Set(mode),
        locked: ActiveValue::Set(locked),
        created_at: ActiveValue::Set(Utc::now().to_rfc3339()),
        updated_at: ActiveValue::Set(Utc::now().to_rfc3339()),
        ..Default::default()
    };

    let inserted_mix = new_mix.insert(db).await?;
    Ok(inserted_mix)
}

pub async fn get_all_mixes(
    db: &DatabaseConnection,
) -> Result<Vec<mixes::Model>, Box<dyn std::error::Error>> {
    use mixes::Entity as MixEntity;

    let mixes = MixEntity::find().all(db).await?;
    Ok(mixes)
}

pub async fn get_mix_by_id(
    db: &DatabaseConnection,
    id: i32,
) -> Result<mixes::Model, Box<dyn std::error::Error>> {
    use mixes::Entity as MixEntity;

    let mix = MixEntity::find_by_id(id).one(db).await?;
    match mix {
        Some(m) => Ok(m),
        None => Err("Mix not found".into()),
    }
}

pub async fn update_mix(
    db: &DatabaseConnection,
    id: i32,
    name: Option<String>,
    group: Option<String>,
    scriptlet_mode: Option<bool>,
    mode: Option<i32>,
    locked: Option<bool>,
) -> Result<mixes::Model, Box<dyn std::error::Error>> {
    use mixes::Entity as MixEntity;

    let mut mix: mixes::ActiveModel = MixEntity::find_by_id(id)
        .one(db)
        .await?
        .ok_or("Mix not found")?
        .into();

    if let Some(name) = name {
        mix.name = ActiveValue::Set(name);
    }
    if let Some(group) = group {
        mix.group = ActiveValue::Set(group);
    }
    if let Some(scriptlet_mode) = scriptlet_mode {
        mix.scriptlet_mode = ActiveValue::Set(scriptlet_mode);
    }
    if let Some(mode) = mode {
        mix.mode = ActiveValue::Set(mode);
    }
    if let Some(locked) = locked {
        mix.locked = ActiveValue::Set(locked);
    }

    mix.updated_at = ActiveValue::Set(Utc::now().to_rfc3339());
    let updated_mix = mix.update(db).await?;
    Ok(updated_mix)
}

pub async fn delete_mix(
    db: &DatabaseConnection,
    id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    use mixes::Entity as MixEntity;

    let mix = MixEntity::find_by_id(id).one(db).await?;
    if let Some(m) = mix {
        m.delete(db).await?;
        Ok(())
    } else {
        Err("Mix not found".into())
    }
}

pub async fn replace_mix_queries(
    db: &DatabaseConnection,
    mix_id: i32,
    operator_parameters: Vec<(String, String)>,
    group: Option<i32>,
) -> Result<Vec<mix_queries::Model>> {
    use mix_queries::ActiveModel;
    use mix_queries::Entity as MixQueryEntity;

    let txn = db.begin().await?;
    let mut results = Vec::new();
    let mut existing_ids = Vec::new();

    for (operator, parameter) in &operator_parameters {
        let mix_query = MixQueryEntity::find()
            .filter(mix_queries::Column::MixId.eq(mix_id))
            .filter(mix_queries::Column::Operator.eq(operator))
            .filter(mix_queries::Column::Parameter.eq(parameter))
            .one(&txn)
            .await?;

        let mut active_model = if let Some(existing_mix_query) = mix_query {
            existing_ids.push(existing_mix_query.id);
            existing_mix_query.into()
        } else {
            ActiveModel {
                mix_id: ActiveValue::Set(mix_id),
                operator: ActiveValue::Set(operator.clone()),
                parameter: ActiveValue::Set(parameter.clone()),
                group: ActiveValue::Set(group.unwrap_or_default()),
                created_at: ActiveValue::Set(Utc::now().to_rfc3339()),
                ..Default::default()
            }
        };

        if let Some(group) = group {
            active_model.group = ActiveValue::Set(group);
        }

        active_model.updated_at = ActiveValue::Set(Utc::now().to_rfc3339());

        let result = if active_model.id.is_set() {
            active_model.update(&txn).await?
        } else {
            active_model.insert(&txn).await?
        };

        results.push(result);
    }

    let mut operator_parameter_conditions = Condition::any();
    for (operator, parameter) in &operator_parameters {
        operator_parameter_conditions = operator_parameter_conditions.add(
            Condition::all()
                .add(mix_queries::Column::Operator.eq(operator.clone()))
                .add(mix_queries::Column::Parameter.eq(parameter.clone())),
        );
    }

    let delete_condition = Condition::all()
        .add(mix_queries::Column::MixId.eq(mix_id))
        .add(Condition::not(operator_parameter_conditions));

    MixQueryEntity::delete_many()
        .filter(delete_condition)
        .exec(&txn)
        .await?;

    txn.commit().await?;

    Ok(results)
}

pub async fn get_all_mix_queries(
    db: &DatabaseConnection,
) -> Result<Vec<mix_queries::Model>, Box<dyn std::error::Error>> {
    use mix_queries::Entity as MixQueryEntity;

    let mix_queries = MixQueryEntity::find().all(db).await?;
    Ok(mix_queries)
}

pub async fn get_mix_query_by_id(
    db: &DatabaseConnection,
    id: i32,
) -> Result<mix_queries::Model, Box<dyn std::error::Error>> {
    use mix_queries::Entity as MixQueryEntity;

    let mix_query = MixQueryEntity::find_by_id(id).one(db).await?;
    match mix_query {
        Some(mq) => Ok(mq),
        None => Err("Mix query not found".into()),
    }
}

pub async fn delete_mix_query(
    db: &DatabaseConnection,
    id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    use mix_queries::Entity as MixQueryEntity;

    let mix_query = MixQueryEntity::find_by_id(id).one(db).await?;
    if let Some(mq) = mix_query {
        mq.delete(db).await?;
        Ok(())
    } else {
        Err("Mix query not found".into())
    }
}

pub async fn get_unique_mix_groups(
    db: &DatabaseConnection,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    use mixes::Entity as PlaylistEntity;

    let unique_groups: Vec<String> = PlaylistEntity::find()
        .select_only()
        .column(mixes::Column::Group)
        .distinct()
        .into_tuple::<String>()
        .all(db)
        .await?;

    Ok(unique_groups)
}

#[derive(Debug)]
enum QueryOperator {
    LibArtist(i32),
    LibAlbum(i32),
    LibPlaylist(i32),
    LibTrack(i32),
    LibDirectoryDeep(String),
    LibDirectoryShallow(String),
    SortLastModified(bool),
    SortDuration(bool),
    SortPlayedthrough(bool),
    SortSkipped(bool),
    FilterLiked(bool),
    PipeLimit(u64),
    PipeRecommend(i32),
    Unknown(String),
}

fn parse_parameter<T>(parameter: &str, operator: &str) -> Option<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Debug,
{
    match parameter.parse::<T>() {
        Ok(x) => Some(x),
        Err(_) => {
            warn!(
                "Unable to parse the parameter of operator: {}({})",
                operator, parameter
            );
            None
        }
    }
}

pub async fn add_item_to_mix(
    db: &DatabaseConnection,
    mix_id: i32,
    operator: String,
    parameter: String,
) -> Result<mix_queries::Model> {
    use mix_queries::ActiveModel;
    use mix_queries::Entity as MixQueryEntity;

    // Check for duplicates: check if there is already an entry with the same mix_id, operator, and parameter in the mix_queries table
    let existing_item = MixQueryEntity::find()
        .filter(mix_queries::Column::MixId.eq(mix_id))
        .filter(mix_queries::Column::Operator.eq(operator.clone()))
        .filter(mix_queries::Column::Parameter.eq(parameter.clone()))
        .one(db)
        .await?;

    if let Some(existing_item) = existing_item {
        // If the entry already exists, return the existing entry directly
        Ok(existing_item)
    } else {
        // If the entry does not exist, insert a new entry
        let new_mix_query = ActiveModel {
            mix_id: ActiveValue::Set(mix_id),
            operator: ActiveValue::Set(operator),
            parameter: ActiveValue::Set(parameter),
            created_at: ActiveValue::Set(Utc::now().to_rfc3339()),
            updated_at: ActiveValue::Set(Utc::now().to_rfc3339()),
            ..Default::default()
        };

        let inserted_mix_query = new_mix_query.insert(db).await?;
        Ok(inserted_mix_query)
    }
}

fn parse_query(query: &(String, String)) -> QueryOperator {
    let (operator, parameter) = query;
    match operator.as_str() {
        "lib::artist" => parse_parameter::<i32>(parameter, operator)
            .map(QueryOperator::LibArtist)
            .unwrap_or(QueryOperator::Unknown(operator.clone())),
        "lib::album" => parse_parameter::<i32>(parameter, operator)
            .map(QueryOperator::LibAlbum)
            .unwrap_or(QueryOperator::Unknown(operator.clone())),
        "lib::playlist" => parse_parameter::<i32>(parameter, operator)
            .map(QueryOperator::LibPlaylist)
            .unwrap_or(QueryOperator::Unknown(operator.clone())),
        "lib::track" => parse_parameter::<i32>(parameter, operator)
            .map(QueryOperator::LibTrack)
            .unwrap_or(QueryOperator::Unknown(operator.clone())),
        "lib::directory.deep" => QueryOperator::LibDirectoryDeep(parameter.clone()),
        "lib::directory.shallow" => QueryOperator::LibDirectoryShallow(parameter.clone()),
        "sort::last_modified" => parse_parameter::<bool>(parameter, operator)
            .map(QueryOperator::SortLastModified)
            .unwrap_or(QueryOperator::Unknown(operator.clone())),
        "sort::duration" => parse_parameter::<bool>(parameter, operator)
            .map(QueryOperator::SortDuration)
            .unwrap_or(QueryOperator::Unknown(operator.clone())),
        "sort::playedthrough" => parse_parameter::<bool>(parameter, operator)
            .map(QueryOperator::SortPlayedthrough)
            .unwrap_or(QueryOperator::Unknown(operator.clone())),
        "sort::skipped" => parse_parameter::<bool>(parameter, operator)
            .map(QueryOperator::SortSkipped)
            .unwrap_or(QueryOperator::Unknown(operator.clone())),
        "filter::liked" => parse_parameter::<bool>(parameter, operator)
            .map(QueryOperator::FilterLiked)
            .unwrap_or(QueryOperator::Unknown(operator.clone())),
        "pipe::limit" => parse_parameter::<u64>(parameter, operator)
            .map(QueryOperator::PipeLimit)
            .unwrap_or(QueryOperator::Unknown(operator.clone())),
        "pipe::recommend" => parse_parameter::<i32>(parameter, operator)
            .map(QueryOperator::PipeRecommend)
            .unwrap_or(QueryOperator::Unknown(operator.clone())),
        _ => QueryOperator::Unknown(operator.clone()),
    }
}

fn apply_join_filter(
    query: Select<media_files::Entity>,
    filter_liked: Option<bool>,
    sort_playedthrough_asc: Option<bool>,
    sort_skipped_asc: Option<bool>,
) -> Select<media_files::Entity> {
    if filter_liked.is_some() || sort_playedthrough_asc.is_some() || sort_skipped_asc.is_some() {
        query.join(
            sea_orm::JoinType::LeftJoin,
            media_file_stats::Entity::belongs_to(media_files::Entity)
                .from(media_file_stats::Column::MediaFileId)
                .to(media_files::Column::Id)
                .into(),
        )
    } else {
        query
    }
}

// Macro to handle sorting
macro_rules! apply_sorting_macro {
    ($query:expr, $column:expr, $sort_option:expr) => {
        if let Some(asc) = $sort_option {
            $query = $query.order_by($column, if asc { Order::Asc } else { Order::Desc });
        }
    };
}

// Macro to handle cursor sorting
macro_rules! apply_cursor_sorting_macro {
    ($query:expr, $cursor_by:expr, $column:expr, $sort_option:expr, $final_asc:expr) => {
        if let Some(asc) = $sort_option {
            $cursor_by = $query.clone().cursor_by($column);
            $final_asc = asc;
        }
    };
}

// Macro to handle subquery filters
macro_rules! add_subquery_filter {
    ($or_condition:expr, $ids:expr, $entity:ty, $column:expr, $file_column:expr) => {
        if !$ids.is_empty() {
            let subquery = <$entity>::find()
                .select_only()
                .filter($column.is_in($ids))
                .column($file_column)
                .into_query();

            $or_condition =
                $or_condition.add(Expr::col(media_files::Column::Id).in_subquery(subquery));
        }
    };
}

pub async fn query_mix_media_files(
    main_db: &DatabaseConnection,
    recommend_db: &RecommendationDbConnection,
    queries: Vec<(String, String)>,
    cursor: usize,
    page_size: usize,
) -> Result<Vec<media_files::Model>> {
    let mut artist_ids: Vec<i32> = vec![];
    let mut album_ids: Vec<i32> = vec![];
    let mut playlist_ids: Vec<i32> = vec![];
    let mut track_ids: Vec<i32> = vec![];
    let mut directories_deep: Vec<String> = vec![];
    let mut directories_shallow: Vec<String> = vec![];

    let mut sort_last_modified_asc: Option<bool> = None;
    let mut sort_duration_asc: Option<bool> = None;
    let mut sort_playedthrough_asc: Option<bool> = None;
    let mut sort_skipped_asc: Option<bool> = None;

    let mut filter_liked: Option<bool> = None;
    let mut pipe_limit: Option<u64> = None;
    let mut pipe_recommend: Option<i32> = None;

    for query in queries {
        match parse_query(&query) {
            QueryOperator::LibArtist(id) => artist_ids.push(id),
            QueryOperator::LibAlbum(id) => album_ids.push(id),
            QueryOperator::LibPlaylist(id) => playlist_ids.push(id),
            QueryOperator::LibTrack(id) => track_ids.push(id),
            QueryOperator::LibDirectoryDeep(dir) => directories_deep.push(dir),
            QueryOperator::LibDirectoryShallow(dir) => directories_shallow.push(dir),
            QueryOperator::SortLastModified(asc) => sort_last_modified_asc = Some(asc),
            QueryOperator::SortDuration(asc) => sort_duration_asc = Some(asc),
            QueryOperator::SortPlayedthrough(asc) => sort_playedthrough_asc = Some(asc),
            QueryOperator::SortSkipped(asc) => sort_skipped_asc = Some(asc),
            QueryOperator::FilterLiked(liked) => filter_liked = Some(liked),
            QueryOperator::PipeLimit(limit) => pipe_limit = Some(limit),
            QueryOperator::PipeRecommend(recommend) => pipe_recommend = Some(recommend),
            QueryOperator::Unknown(op) => warn!("Unknown operator: {}", op),
        }
    }

    if pipe_recommend.is_some() && cursor > 0 {
        return Ok([].to_vec());
    }

    // Base query for media_files
    let mut query = media_files::Entity::find();

    // Create an OR condition to hold all the subconditions
    let mut or_condition = Condition::any();

    // Filter by artist_ids if provided
    add_subquery_filter!(
        or_condition,
        artist_ids,
        media_file_artists::Entity,
        media_file_artists::Column::ArtistId,
        media_file_artists::Column::MediaFileId
    );

    // Filter by album_ids if provided
    add_subquery_filter!(
        or_condition,
        album_ids,
        media_file_albums::Entity,
        media_file_albums::Column::AlbumId,
        media_file_albums::Column::MediaFileId
    );

    // Filter by playlist_ids if provided
    add_subquery_filter!(
        or_condition,
        playlist_ids,
        media_file_playlists::Entity,
        media_file_playlists::Column::PlaylistId,
        media_file_playlists::Column::MediaFileId
    );

    // Filter by track_ids if provided
    if !track_ids.is_empty() {
        or_condition = or_condition.add(Expr::col(media_files::Column::Id).is_in(track_ids));
    }

    // Filter by directories if provided
    if !directories_deep.is_empty() {
        let mut dir_conditions = Condition::any();
        for dir in directories_deep {
            let dir = dir.strip_prefix('/').unwrap_or(&dir);

            dir_conditions = dir_conditions.add(
                Expr::col(media_files::Column::Directory)
                    .eq(dir)
                    .or(Expr::col(media_files::Column::Directory).like(format!("{}/%", dir))),
            );
        }
        or_condition = or_condition.add(dir_conditions);
    }

    // Filter by directories if provided
    if !directories_shallow.is_empty() {
        let mut dir_conditions = Condition::any();
        for dir in directories_shallow {
            let dir = dir.strip_prefix('/').unwrap_or(&dir);

            dir_conditions = dir_conditions.add(Expr::col(media_files::Column::Directory).eq(dir));
        }
        or_condition = or_condition.add(dir_conditions);
    }

    if let Some(liked) = filter_liked {
        query = query.filter(
            Condition::all()
                .add(or_condition)
                .add(media_file_stats::Column::Liked.eq(liked)),
        );
    } else {
        query = query.filter(or_condition);
    }

    // Join with media_file_stats table for sorting by playedthrough and skipped, and filtering by liked
    query = apply_join_filter(
        query,
        filter_liked,
        sort_playedthrough_asc,
        sort_skipped_asc,
    );

    if let Some(recommend_group) = pipe_recommend {
        apply_sorting_macro!(
            query,
            media_files::Column::LastModified,
            sort_last_modified_asc
        );
        apply_sorting_macro!(query, media_files::Column::Duration, sort_duration_asc);

        if let Some(asc) = sort_playedthrough_asc {
            query = query.order_by(
                media_file_stats::Column::PlayedThrough,
                if asc { Order::Asc } else { Order::Desc },
            );
        }

        if let Some(asc) = sort_skipped_asc {
            query = query.order_by(
                media_file_stats::Column::Skipped,
                if asc { Order::Asc } else { Order::Desc },
            );
        }

        if let Some(query_limit) = pipe_limit {
            query = query.limit(query_limit);
        }

        let file_ids = query
            .select_only()
            .column(media_files::Column::Id)
            .distinct()
            .into_tuple::<i32>()
            .all(main_db)
            .await
            .with_context(|| "Failed to query file ids for recommendation")?;

        let virtual_point: [f32; 61] = if recommend_group >= 0 {
            get_percentile_analysis_result(
                main_db,
                1.0 / (9 + 2) as f64 * (recommend_group + 1) as f64,
            )
            .await
            .with_context(|| "Failed to query percentile data")?
        } else {
            get_centralized_analysis_result(main_db, file_ids)
                .await
                .with_context(|| "Failed to query centralized data")?
                .into()
        };

        let recommend_n = if let Some(query_limit) = pipe_limit {
            query_limit
        } else {
            30
        };

        let file_ids =
            get_recommendation_by_parameter(recommend_db, virtual_point, recommend_n as usize)
                .with_context(|| "Failed to get recommendation by parameters")?
                .into_iter()
                .map(|x| x.0 as i32)
                .collect::<Vec<i32>>();

        return Ok(get_files_by_ids(main_db, &file_ids).await?);
    }

    // Use cursor pagination
    let mut cursor_by = query.clone().cursor_by(media_files::Column::Id);
    let mut final_asc = true;

    apply_cursor_sorting_macro!(
        query,
        cursor_by,
        media_files::Column::LastModified,
        sort_last_modified_asc,
        final_asc
    );
    apply_cursor_sorting_macro!(
        query,
        cursor_by,
        media_files::Column::Duration,
        sort_duration_asc,
        final_asc
    );
    apply_cursor_sorting_macro!(
        query,
        cursor_by,
        media_file_stats::Column::PlayedThrough,
        sort_playedthrough_asc,
        final_asc
    );
    apply_cursor_sorting_macro!(
        query,
        cursor_by,
        media_file_stats::Column::Skipped,
        sort_skipped_asc,
        final_asc
    );

    if let Some(limit) = pipe_limit {
        if cursor as u64 >= limit {
            return Ok(vec![]);
        }
    }

    let final_page_size = if let Some(limit) = pipe_limit {
        (limit - cursor as u64).min(page_size as u64)
    } else {
        page_size as u64
    };

    let media_files = (if final_asc {
        cursor_by.after(cursor as i32).first(final_page_size)
    } else {
        cursor_by
            .desc()
            .before(cursor as i32)
            .first(final_page_size)
    })
    .all(main_db)
    .await?;

    Ok(media_files)
}