use tokio::sync;

#[derive(PartialEq, Eq, Debug)]
pub enum MsgSrv {
    File(/* path: */ String, /* content: */ String),
    Exit(),
}

#[derive(Debug)]
pub enum MsgBuilder {
    File(
        /* path: */ String,
        /* result: */ sync::oneshot::Sender<(Option<String>, /* all_files: */ Vec<String>)>,
    ),
    Exit(),
}

#[derive(Debug)]
pub enum MsgInternalBuilder {
    FileModified(/* path: */ String),
    FileDeleted(/* path */ String),
    Exit(),
}
