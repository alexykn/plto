use anyhow::Result;
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::{
    ExecutionContext,
    config::PathExcludeConfig,
    config::PathReplacementConfig,
    context::{ContextMap, TemplateContext},
    names::ProjectNameSet,
};

use self::builder::WorkspaceBuilder;
use self::rendered::RenderedWorkspace;

pub(crate) mod builder;
pub(crate) mod content;
pub(crate) mod path_exclude;
pub(crate) mod path_rewrite;
pub(crate) mod rendered;

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceRenderContext {
    pub(crate) template_context: TemplateContext,
    pub(crate) path_replacements: BTreeMap<String, PathReplacementConfig>,
    pub(crate) path_excludes: BTreeMap<String, PathExcludeConfig>,
    pub(crate) source_path: PathBuf,
}

impl From<&ExecutionContext> for WorkspaceRenderContext {
    fn from(exec_ctx: &ExecutionContext) -> Self {
        Self {
            template_context: build_template_context(exec_ctx),
            path_replacements: exec_ctx.config.path.replace.clone(),
            path_excludes: exec_ctx.config.path.exclude.clone(),
            source_path: exec_ctx.source_path.clone(),
        }
    }
}

fn build_template_context(exec_ctx: &ExecutionContext) -> TemplateContext {
    build_template_context_parts(
        &exec_ctx.project_name,
        &exec_ctx.config,
        exec_ctx.context_overrides.clone(),
    )
}

pub(crate) fn build_template_context_parts(
    project_name: &str,
    config: &crate::config::Config,
    context_overrides: ContextMap,
) -> TemplateContext {
    let mut template_context = TemplateContext::new();
    let project_names = ProjectNameSet::derive(project_name);

    project_names.insert_context(&mut template_context);
    if let Ok(plugin_config) = serde_json::to_value(&config.plugins) {
        template_context.insert_value("config", serde_json::json!({ "plugins": plugin_config }));
    }

    template_context.merge(config.template.context.clone());
    template_context.merge(context_overrides);
    template_context
}

pub(crate) fn render_workspace(ctx: &WorkspaceRenderContext) -> Result<RenderedWorkspace> {
    WorkspaceBuilder::from_source(&ctx.source_path)?
        .exclude_paths(&ctx.template_context, &ctx.path_excludes)?
        .rewrite_paths(&ctx.template_context, &ctx.path_replacements)?
        .render_templates(&ctx.template_context)?
        .build()
}
