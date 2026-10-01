use serde::Serialize;
use std::sync::Mutex;

#[derive(Clone, Serialize)]
pub(crate) struct Attempt {
    pub args: Vec<String>,
    pub errors: String,
}

#[derive(Default)]
pub(crate) struct Inspector(pub Mutex<Vec<Attempt>>);

impl Inspector {
    pub fn start(&self, args: &[String]) {
        if let Ok(mut attempts) = self.0.lock() {
            if attempts.len() >= 16 { attempts.remove(0); }
            attempts.push(Attempt { args: args.to_vec(), errors: String::new() });
        }
    }
    pub fn errors(&self, lines: &[String]) {
        if let Ok(mut attempts) = self.0.lock() {
            if let Some(last) = attempts.last_mut() {
                last.errors = lines.iter().rev().take(40).cloned().collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
                if last.errors.len()>16000 { let mut end=16000;while !last.errors.is_char_boundary(end){end-=1}last.errors.truncate(end); }
            }
        }
    }
}

#[tauri::command]
pub(crate) fn job_inspector(state: tauri::State<'_, super::JobState>) -> Vec<Attempt> {
    state.inspector.0.lock().map(|attempts| attempts.clone()).unwrap_or_default()
}

#[cfg(test)]
mod tests{
    #[test]
    fn inspector_bounds_attempts_and_preserves_last_error(){
        let inspector=super::Inspector::default();
        for index in 0..20{inspector.start(&[format!("attempt-{index}")]);}
        inspector.errors(&["last failure".into()]);
        let attempts=inspector.0.lock().unwrap();
        assert_eq!(attempts.len(),16);
        assert_eq!(attempts[0].args[0],"attempt-4");
        assert_eq!(attempts[15].errors,"last failure");
    }
}
