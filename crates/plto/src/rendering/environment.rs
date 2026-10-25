use minijinja::syntax::SyntaxConfig;
use minijinja::{Environment, UndefinedBehavior};

use super::filters;

pub(crate) fn new_template_environment() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.set_syntax(
        SyntaxConfig::builder()
            .keep_trailing_newline(true)
            .build()
            .expect("default delimiters are valid"),
    );
    filters::register_all(&mut env);
    env
}
