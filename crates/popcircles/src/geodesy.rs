// The earth model, stated once for the whole program: a sphere of the IUGG mean radius, with
// distances as great-circle arcs on it. Nothing here is ellipsoidal, so a result may differ from a
// WGS 84 geodesic by a few tenths of a percent.
pub const EARTH_RADIUS_KM: f64 = 6371.0088;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}
