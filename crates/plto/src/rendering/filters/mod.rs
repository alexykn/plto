use minijinja::Environment;

mod regex;

pub(crate) fn register_all(env: &mut Environment<'_>) {
    regex::register(env);
}
