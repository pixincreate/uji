include!(concat!(env!("OUT_DIR"), "/bundled.rs"));

pub(super) fn file(path: &str) -> Option<&'static str> {
    FILES
        .iter()
        .find(|(name, _)| *name == path)
        .map(|(_, source)| *source)
}

pub(super) fn name(path: &str) -> String {
    format!("{ROOT}/{path}")
}
