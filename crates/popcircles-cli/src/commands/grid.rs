// Grid geometry with no raster behind it: the six declared numbers, checked and described.

use popcircles::report::{Envelope, GridSummary};

use crate::args::GridArgs;
use crate::commands::serialised;
use crate::failure::Failure;

pub(crate) fn describe_grid(args: GridArgs) -> Result<String, Failure> {
    let grid = args.grid().map_err(|error| Failure::grid(&error))?;
    serialised(serde_json::to_string(&Envelope::new(GridSummary::from(
        &grid,
    ))))
}
