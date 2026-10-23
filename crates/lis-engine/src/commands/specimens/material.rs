//! Audited, bounded material and preservation history.
use super::MATERIAL_READ;
use crate::pipeline::access;
use crate::{
    Engine,
    registry::{Access, CommandSpec, spec},
};
use lis_core::{Capability, CommandError};
use lis_specimens::{
    ContainerId, ExcursionId, ExcursionPage, ExcursionView, MaterialHistoryPage,
    MaterialHistorySnapshot,
};
use serde::Deserialize;
use ts_rs::TS;

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetContainerHistoryInput {
    pub container_id: ContainerId,
    #[serde(default)]
    pub offset: u32,
    #[serde(default)]
    pub snapshot: Option<MaterialHistorySnapshot>,
}
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListExcursionsInput {
    #[serde(default)]
    pub offset: u32,
    #[serde(default)]
    pub snapshot: Option<String>,
}
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetExcursionInput {
    pub excursion_id: ExcursionId,
}

pub(crate) const GET_CONTAINER_HISTORY: CommandSpec = spec(
    "get_container_history",
    "specimen.view_material_history",
    MATERIAL_READ,
);
const EXCURSION_READ: Access =
    Access::EitherCapability(Capability::Accession, Capability::ManageQuality);
pub(crate) const LIST_EXCURSIONS: CommandSpec = spec(
    "list_excursions",
    "specimen.list_excursions",
    EXCURSION_READ,
);
pub(crate) const GET_EXCURSION: CommandSpec =
    spec("get_excursion", "specimen.view_excursion", EXCURSION_READ);
impl Engine {
    pub async fn get_container_history(
        &self,
        input: GetContainerHistoryInput,
    ) -> Result<MaterialHistoryPage, CommandError> {
        self.run(&GET_CONTAINER_HISTORY, |call| async move {
            self.audited_read(
                &call,
                move |conn| {
                    let mut page = lis_specimens::get_container_history(
                        conn,
                        input.container_id,
                        input.offset,
                        input.snapshot,
                    )?;
                    super::names::material_history(conn, &mut page)?;
                    Ok::<_, lis_specimens::SpecimenError>(page)
                },
                move |_| {
                    access(GET_CONTAINER_HISTORY.action).record("container", input.container_id)
                },
            )
            .await
        })
        .await
    }
    pub async fn list_excursions(
        &self,
        input: ListExcursionsInput,
    ) -> Result<ExcursionPage, CommandError> {
        self.run(&LIST_EXCURSIONS, |call| async move {
            self.audited_read(
                &call,
                move |conn| {
                    let mut page =
                        lis_specimens::list_excursions(conn, input.offset, input.snapshot)?;
                    super::names::excursions(conn, &mut page.items)?;
                    Ok::<_, lis_specimens::SpecimenError>(page)
                },
                |_| access(LIST_EXCURSIONS.action),
            )
            .await
        })
        .await
    }
    pub async fn get_excursion(
        &self,
        input: GetExcursionInput,
    ) -> Result<ExcursionView, CommandError> {
        self.run(&GET_EXCURSION, |call| async move {
            self.audited_read(
                &call,
                move |conn| {
                    let mut detail = lis_specimens::excursion(conn, input.excursion_id)?;
                    super::names::excursions(conn, std::slice::from_mut(&mut detail))?;
                    Ok::<_, lis_specimens::SpecimenError>(detail)
                },
                move |_| access(GET_EXCURSION.action).record("excursion", input.excursion_id),
            )
            .await
        })
        .await
    }
}
