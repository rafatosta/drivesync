//! Navigation history stores known paths without interpreting folder IDs.
use super::model::*;
#[derive(Default)]
pub(super) struct Navigation {
    pub history: Vec<Vec<RemoteFolder>>,
    pub cursor: usize,
}
impl Navigation {
    pub fn path(&self) -> Vec<RemoteFolder> {
        self.history[self.cursor].clone()
    }
    pub fn visit(&mut self, path: Vec<RemoteFolder>) {
        if self.history.get(self.cursor) == Some(&path) {
            return;
        }
        self.history.truncate(self.cursor + 1);
        self.history.push(path);
        self.cursor = self.history.len() - 1;
    }
    pub fn step(&mut self, forward: bool) {
        if forward && self.cursor + 1 < self.history.len() {
            self.cursor += 1;
        } else if !forward {
            self.cursor = self.cursor.saturating_sub(1);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn folder(id: &str) -> RemoteFolder {
        RemoteFolder {
            id: RemoteFolderId(id.into()),
            name: id.into(),
            parent_id: None,
        }
    }
    #[test]
    fn branching_discards_forward_history_and_preserves_opaque_ids() {
        let mut nav = Navigation::default();
        nav.visit(vec![folder("opaque:/root")]);
        nav.visit(vec![folder("opaque:/root"), folder("child")]);
        nav.step(false);
        nav.visit(vec![folder("opaque:/root"), folder("other")]);
        nav.step(true);
        assert_eq!(nav.history.len(), 2);
        assert_eq!(nav.path()[1].id.0, "other");
        nav.step(false);
        nav.step(false);
        assert_eq!(nav.cursor, 0);
    }
}
