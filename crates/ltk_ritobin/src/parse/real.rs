use std::{
    cell::Cell,
    fmt::{self, Display},
    sync::{Arc, Mutex},
};

#[salsa::db]
#[derive(Clone)]
#[cfg_attr(not(test), derive(Default))]
pub struct CalcDatabaseImpl {
    storage: salsa::Storage<Self>,

    // The logs are only used for testing and demonstrating reuse:
    #[cfg(test)]
    logs: Arc<Mutex<Option<Vec<String>>>>,
}

#[cfg(test)]
impl Default for CalcDatabaseImpl {
    fn default() -> Self {
        let logs = <Arc<Mutex<Option<Vec<String>>>>>::default();
        Self {
            storage: salsa::Storage::new(Some(Box::new({
                let logs = logs.clone();
                move |event| {
                    eprintln!("Event: {event:?}");
                    // Log interesting events, if logging is enabled
                    if let Some(logs) = &mut *logs.lock().unwrap() {
                        // only log interesting events
                        if let salsa::EventKind::WillExecute { .. } = event.kind {
                            logs.push(format!("Event: {event:?}"));
                        }
                    }
                }
            }))),
            logs,
        }
    }
}

#[salsa::db]
impl salsa::Database for CalcDatabaseImpl {}

#[salsa::input(debug)]
pub struct SourceProgram {
    #[returns(ref)]
    pub text: String,
}

#[salsa::tracked(debug)]
pub struct RitobinFile<'db> {
    #[tracked]
    #[returns(ref)]
    pub statements: Vec<Statement<'db>>,
}

#[salsa::interned(debug)]
pub struct PropertyName<'db> {
    #[returns(ref)]
    pub text: String,
}

#[derive(PartialEq, Debug, Hash, salsa::Update)]
pub struct Statement<'db> {
    pub span: Span<'db>,
    pub name: PropertyName<'db>,
    pub value: BinProperty,
}

#[salsa::accumulator]
#[derive(Debug)]
#[allow(dead_code)] // Debug impl uses them
pub struct Diagnostic {
    pub start: usize,
    pub end: usize,
    pub message: String,
}
impl Diagnostic {
    pub fn new(start: usize, end: usize, message: String) -> Self {
        Diagnostic {
            start,
            end,
            message,
        }
    }

    // #[cfg(test)]
    // pub fn render(&self, db: &dyn crate::Db, src: SourceProgram) -> String {
    //     use annotate_snippets::*;
    //     let line_start = src.text(db)[..self.start].lines().count() + 1;
    //     Renderer::plain()
    //         .render(
    //             Level::Error.title(&self.message).snippet(
    //                 Snippet::source(src.text(db))
    //                     .line_start(line_start)
    //                     .origin("input")
    //                     .fold(true)
    //                     .annotation(Level::Error.span(self.start..self.end).label("here")),
    //             ),
    //         )
    //         .to_string()
    // }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, PartialEq, Debug, Hash, salsa::Update)]
pub struct BinProperty {
    pub name_hash: u32,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub value: ltk_meta::PropertyValueEnum,
}

#[salsa::tracked(debug)]
pub struct Span<'db> {
    #[tracked]
    pub start: usize,
    #[tracked]
    pub end: usize,
}
