# health-check

A health check executable that checks for common server failure modes
and sends out notification to the communication platform (currently
only supports Slack).

It's recommended to use this executable as the entrypoint in your
Docker container.

# Why

You might want to use this because:

- It has [pid1](https://crates.io/crates/pid1) integration and ensure proper cleanup of resources.
- Detects common server failure modes.
- Sends out log crash report stdout/stderr to the configured platform.

# CLI Usage

``` shellsession
health-check --help
Usage: health-check [OPTIONS] --app-description <APP_DESCRIPTION> --app-version <APP_VERSION> --notification-message <NOTIFICATION_MESSAGE> <COMMAND> [ARGS]...

Arguments:
  <COMMAND>  Process to run
  [ARGS]...  Arguments to the process

Options:
      --task-output-timeout <TASK_OUTPUT_TIMEOUT>
          Seconds to wait for output before killing the task
      --slack-webhook <SLACK_WEBHOOK>
          Slack Webhook for notification [env: HEALTH_CHECK_SLACK_WEBHOOK=]
      --app-description <APP_DESCRIPTION>
          Application description
      --app-version <APP_VERSION>
          Applicationv version [env: HEALTH_CHECK_APP_VERSION=]
      --notification-message <NOTIFICATION_MESSAGE>
          Notification Content [env: HEALTH_CHECK_NOTIFICATION_MESSAGE=]
      --image-url <IMAGE_URL>
          Image url for notification message [env: HEALTH_CHECK_IMAGE_URL=]
      --can-exit
          Is the child process allowed to exit on its own? By default it is false
  -h, --help
          Print help
```

Note that currently `--slack-webhook` is optional because we might
want to extend `health-check` executable to support multiple
communication platform like Discord or Microsoft teams.

# Dockerfile usage

``` dockerfile
FROM alpine:3.20.1

ADD --chmod=755 https://github.com/fpco/health-check/releases/download/v0.4.0/health-check-x86_64-unknown-linux-musl /usr/bin/health-check

ENTRYPOINT [ "/usr/bin/health-check" ]

CMD [ "--app-description", "My Unicorn App", "--app-version", "v0.1", "--notification-context", "Test infrastructure", "--task-output-timeout", "250", "/usr/bin/unicorn_app" ]
```
