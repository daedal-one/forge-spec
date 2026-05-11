pub mod agent;
pub mod human;
pub mod scope;
pub mod source;

/// Render target format.
#[derive(Debug, Clone)]
pub enum RenderTarget {
    Human,
    Agent,
}
