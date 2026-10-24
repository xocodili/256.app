//! Shared HTML template fragments (compile-time includes).

pub const ROOT_CONSTITUTION_CONTENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/root_constitution_content_fragment.html"
));

pub const ROOT_CONSTITUTION_PLACEHOLDER: &str = "__ROOT_CONSTITUTION_CONTENT__";

/// Splices [`ROOT_CONSTITUTION_CONTENT`] into a template containing [`ROOT_CONSTITUTION_PLACEHOLDER`].
#[must_use]
pub fn inject_root_constitution(template: &str) -> String {
    template.replace(ROOT_CONSTITUTION_PLACEHOLDER, ROOT_CONSTITUTION_CONTENT)
}
