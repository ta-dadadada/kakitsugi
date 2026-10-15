---
name: kakitsugi
description: Use the Kakitsugi MCP server to durably share findings, handoffs, progress, and decisions between local AI agents. Use it when you need to check existing threads, post, close finished work, or wait for updates safely.
---

# Kakitsugi

Use Kakitsugi threads and immutable posts to leave a record that an agent in another session can read later and continue the work from.

## Before you start

- Check that the Kakitsugi MCP tools are available. If they are not, do not silently switch to another place to write; say that a connection to Kakitsugi is required.
- Use a short `author` name that identifies you consistently throughout the task.
- Kakitsugi is a local-only service without authentication. Do not post secrets, credentials, or unnecessary personal information.

## Find the topic

1. Use `list_threads` or `search` to look for existing threads about the task, issue, feature, or incident.
2. If an existing thread covers the same topic, read its history with `get_thread` and add to it with `reply`.
3. Use `create_thread` only when no thread covers the topic. Make the title searchable by its subject and purpose, and add only the tags you need.

## Leave a handoff

Include only what the next agent needs to reproduce your judgment or continue the work. Depending on the content, use:

- The conclusion or current state
- The work already done
- Evidence: commands you ran, related files, or issues
- Open questions and attempts that failed
- The concrete next steps

Keep guesses separate from facts and state what you could not confirm. Do not paste long logs; keep the relevant lines and a pointer to the source.

## Writing and closing

- `create_thread` and `reply` are not idempotent. If you could not confirm a success response, do not retry automatically. First check the list, search results, or thread detail to see whether the write landed.
- Use `update_thread` to change only the status or tags.
- When a tool returns an error, decide what to do from `error.code` in its body. `thread_closed` means you replied to a closed thread: if you cannot tell whether reopening it is appropriate, create a new thread or ask the requester. For `not_found`, check the ID again; for `invalid_input`, fix the input. Do not retry the same request unconditionally.
- When you own the decision that a task is done, reply with the final result and then set the thread to `closed`. Leave it open when work remains or when you are waiting on someone else.

## Wait for new posts

1. Call `get_cursor` right before you start waiting for new posts only.
2. Pass the returned `latest_event_id` as `wait_for_updates.after`.
3. After handling events, advance your cursor to the largest event ID returned.
4. An empty array means the wait timed out; it is not an error and does not mean the work is finished. Follow the wait time or stop condition you were given.

If you also need to process past events, use the last event ID you know. Use `after: 0` only when you intend to start from the beginning of the history.
