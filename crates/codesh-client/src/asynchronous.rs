use crate::{Client, ClientError, Request};
use codesh_protocol::{ProjectId, ReplyBody, RequestBody};
use std::time::Duration;

impl Client {
    pub fn project_has_sessions_async(&self, project: ProjectId) -> Request<bool> {
        self.request_async(
            RequestBody::ListProjectSessions {
                project,
                after: None,
                revision: None,
            },
            Duration::from_secs(5),
            |body| match body {
                ReplyBody::ProjectSessions(page) => {
                    Ok(!page.sessions.is_empty() || page.next.is_some())
                }
                body => Err(ClientError::UnexpectedReply(Box::new(body))),
            },
        )
    }

    pub fn git_async(
        &self,
        request: codesh_protocol::GitRequest,
    ) -> Request<codesh_protocol::GitReply> {
        let timeout = crate::catalog::git_timeout(&request.action);
        self.request_async(RequestBody::Git(request), timeout, |body| match body {
            ReplyBody::Git(value) => Ok(value),
            body => Err(ClientError::UnexpectedReply(Box::new(body))),
        })
    }

    pub fn files_async(
        &self,
        request: codesh_protocol::FilesRequest,
    ) -> Request<codesh_protocol::FilesReply> {
        let timeout = Duration::from_secs(300);
        self.request_async(RequestBody::Files(request), timeout, |body| match body {
            ReplyBody::Files(value) => Ok(value),
            body => Err(ClientError::UnexpectedReply(Box::new(body))),
        })
    }

    pub fn exec_async(
        &self,
        request: codesh_protocol::ExecRequest,
    ) -> Request<codesh_protocol::ExecResult> {
        let timeout = Duration::from_millis(u64::from(request.timeout_ms) + 10_000);
        self.request_async(RequestBody::Exec(request), timeout, |body| match body {
            ReplyBody::Exec(value) => Ok(value),
            body => Err(ClientError::UnexpectedReply(Box::new(body))),
        })
    }
}
