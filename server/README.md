# mailbox-server

The `mailbox-server` CLI starts an HTTP server that provides a REST API interface for interacting with a mailbox. It stores the messages in an SQLite database. The `mailbox` CLI can interact with this API server as [documented here](../README.md#using-a-remote-database). Another other application can use the same REST interface to read and update messages. For example, suppose you have a script that runs hourly in a Cloudflare Worker that scrapes a webpage for new data. That script can create messages based simply by making calls to the REST endpoint provided by this server.

## Getting started

```sh
$ mailbox-server
$ curl http://localhost:8080/messages
```

## CLI flags

### `--port=<PORT>`

Sets the port that the HTTP server will listen on. The port can also be set with the `$PORT` environment variable.

```sh
$ mailbox-server --port=9000
$ curl http://localhost:9000/messages
```

### `--expose`

Instructs the HTTP server to bind to 0.0.0.0 instead of 127.0.0.1 and accept incoming connections from other machines on the network.

```sh
$ mailbox-server --expose
# On another machine using the server machine's IP address
$ curl http://10.0.0.10:9000/messages
```

### `--token=<TOKEN>`

Causes the HTTP server to expect an `Authorization: Bearer {token}` header containing this token on all requests. Without this arg, no `Authorization` header is required.

```sh
$ mailbox-server --token=0a1b2c3de4f5
$ curl http://localhost:8080/messages -H "Authorization: Bearer 0a1b2c3de4f5"
```

### `--db_file=<DB_FILE>`

Path to the SQLite database file that the server uses to store the messages

```sh
$ mailbox-server --db-file=$HOME/messages.db
$ curl http://localhost:8080/messages
```

## REST API

All responses are in JSON.

### Filters

All API endpoints support filtering the messages that will be interacted with via query string parameters. The available filters are as follows:

- `ids`: comma-separated list of message ids
  - `?ids=3` matches the message with the id `3`
  - `?ids=1,4,5` matches the messages with the ids `1`, `4`, `5`
- `mailbox`: name of the mailbox or parent mailbox
  - `?mailbox=my-script` matches all messages in `my-script` and all children mailboxes like `my-script/mailbox-1` and `my-script/mailbox-2`
  - `?mailbox=my-script/mailbox-1` matches all messages in `my-script/mailbox-1` and all children mailboxes like `my-script/mailbox-1/sub-mailbox` and `my-script/mailbox-2/sub-mailbox`
- `states`: comma-separated list of message states (`unread`, `read`, or `archived`)
  - `?states=read` matches all read messages
  - `?states=unread,archived` matches all unread or archived messages

Filters can also be combined. For example, `?mailbox=other-script&states=read,archived` matches read or archived messages in the `other-script` mailbox.

If no filter is provided, all messages will be interacted with.

### Authorization

If an [authorization token was specified](#--tokentoken) when starting the server, all requests must be sent with an `Authorization` header of `Bearer {token}` where `{token}` is your configured API token.

### Message format

All responses are JSON arrays of message objects. The format of message objects is as follows:

- `id` (integer): the message's id
- `timestamp` (string): the message's creation date in UTC ISO format
- `mailbox` (string): the message's mailbox
- `content` (string): the message's content
- `state` (string): the message's state, which will be one of `unread`, `read`, or `archived`

Example message:

```json
{
  "id": 123,
  "timestamp": "2023-01-01T12:01:02.345678",
  "mailbox": "foo",
  "content": "bar",
  "state": "unread"
}
```

### `GET /messages`

Reads messages. Responds with a JSON array of messages matching the optional message filter ordered by timestamp descending.

### `GET /mailboxes`

Reads mailbox sizes. Responds with an array of JSON objects with a `name` key that is the mailbox name and a `message_count` key that is the number of messages that the mailbox contains. The array elements are ordered by the mailbox name ascending. If an optional message filter is provided, only messages that match the filter are counted towards mailbox sizes.

Example response:

```json
[
  { "name": "mailbox-1", "message_count": 5 },
  { "name": "mailbox-2", "message_count": 2 },
  { "name": "mailbox-2/child", "message_count": 1 },
  { "name": "mailbox-3", "message_count": 4 }
]
```

### `POST /messages`

Creates new messages. Responds with a JSON array of the created messages ordered by timestamp descending. This is the only endpoint that does not accept a message filter. New messages should be posted as JSON in the request body with a `Content-Type` header of `application/json`. The body can either be a single message object or an array of message objects. These are the accepted message object fields:

- `mailbox` (string): the message's mailbox
- `content` (string): the message's content
- `state` (string optional): the message's state, which will be one of `unread`, `read`, or `archived` (defaults to `unread` if omitted)

Example single-message payload:

```json
{
  "mailbox": "my-script",
  "content": "Hello, world!",
  "state": "read"
}
```

Example multi-message payload:

```json
[
  {
    "mailbox": "my-script",
    "content": "Hello, world!"
  },
  {
    "mailbox": "my-script",
    "content": "Hello, universe!",
    "state": "archived"
  }
]
```

### `PUT /messages`

Updates message states. Responds with a JSON array of the updated messages ordered by timestamp descending. Only updates messages matching the optional filter. If no filter is provided, all messages are updated. The new message state should be put as a JSON encoded object with a single field `new_state` in the request body with a `Content-Type` header of `application/json`. `new_state` can have the value `unread`, `read`, or `archived`.

Example request payload to mark all messages as read:

```json
{"new_state": "read"}
```

### `DELETE /messages`

Permanently deletes messages. Responds with a JSON array of the deleted messages ordered by timestamp descending. Only updates messages matching the optional filter. Unlike the other endpoints, if no filter is provided, an error is returned instead of deleting all messages as a safety measure to prevent data loss.
