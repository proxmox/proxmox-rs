use std::fs::{remove_dir_all, write};

use proxmox_rest_server::{
    init_worker_tasks, upid_log_path, upid_read_status, TaskState, WorkerTask,
};
use proxmox_sys::fs::{make_tmp_dir, CreateOptions};

fn upid_read_status_test_helper(log: &str, expected: &TaskState) {
    let task = WorkerTask::new("x", None, "u@pam".into(), false).unwrap().0;
    let upid = task.upid();
    let logfile = upid_log_path(upid).unwrap();
    write(logfile, log).unwrap();

    let status = upid_read_status(upid).unwrap();

    assert!(&status == expected);
}

#[tokio::test]
async fn test_upid_read_status() {
    let tmpdir = make_tmp_dir("/tmp/", None).unwrap();
    init_worker_tasks(tmpdir.clone(), CreateOptions::new()).unwrap();

    upid_read_status_test_helper(
        "2025-09-22T14:31:04+02:00: The quick, brown fox jumps over a lazy dog.\n\
         2025-09-22T14:31:04+02:00: TASK ERROR: Something went wrong",
        &TaskState::Error {
            message: "Something went wrong".into(),
            endtime: 1758544264,
        },
    );

    upid_read_status_test_helper(
        "1970-01-01T01:00:00+01:00: The quick, brown fox jumps over a lazy dog.\n\
         1970-01-01T01:00:01+01:00: TASK ERROR: Lorem ipsum dolor sit amet\n\
         consetetur sadipscing elitr sed diam nonumy eirmod tempor invidunt\n\
         ut labore et dolore magna aliquyam erat, sed diam voluptua.",
        &TaskState::Error {
            message: "Lorem ipsum dolor sit amet".into(),
            endtime: 1,
        },
    );

    upid_read_status_test_helper(
        "2025-09-24T10:56:12+02:00: Lorem ipsum dolor sit amet\n\
         2025-09-24T10:56:13+02:00: consetetur sadipscing elitr\n\
         2025-09-24T10:56:14+02:00: TASK OK",
        &TaskState::OK {
            endtime: 1758704174,
        },
    );

    upid_read_status_test_helper(
        "2025-09-23T12:07:49+02:00: Warning 1\n\
         2025-09-23T12:29:11+02:00: Warning 2\n\
         2025-09-23T12:29:11+02:00: TASK WARNINGS: 2",
        &TaskState::Warning {
            count: 2,
            endtime: 1758623351,
        },
    );

    // Cleanup
    remove_dir_all(tmpdir).unwrap();
}
