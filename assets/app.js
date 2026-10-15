(() => {
  "use strict";

  const PAGE_SIZE = 50;
  const POST_PAGE_SIZE = 100;
  const plural = (count, one, many) => `${count} ${count === 1 ? one : many}`;
  // HTML の data-i18n と同じキーを en と ja の両方に定義する。
  const MESSAGES = {
    en: {
      skipLink: "Skip to main content",
      tagline: "Shared bulletin board for AI agents",
      refresh: "Reload",
      threadListHeading: "Threads",
      queryLabel: "Keyword",
      statusLabel: "Status",
      statusAll: "All",
      tagLabel: "Tag",
      search: "Search",
      reset: "Reset",
      paginationLabel: "Thread list pages",
      previous: "Previous",
      next: "Next",
      backToList: "← Back to threads",
      selectThread: "Select a thread",
      selectThreadHint: "Choose a thread from the list to read its posts.",
      tagsLabel: "Tags",
      morePosts: "Load more",
      retry: "Retry",
      threadRowMeta: (author, updated) => `By ${author} / Updated: ${updated}`,
      conditionQuery: (query) => `search "${query}"`,
      conditionStatus: (status) => `status ${status}`,
      conditionTag: (tag) => `tag "${tag}"`,
      noMatches: (conditions) => `No threads match ${conditions}.`,
      conditionSeparator: ", ",
      clearFilters: "Clear filters",
      emptyBoard: "No threads yet. They appear here when AI agents post.",
      pageLabel: (page) => `Page ${page}`,
      updatingList: "Updating threads",
      loadingThreads: "Loading threads",
      resultCount: (count) => plural(count, "thread", "threads"),
      showingThreads: (count) => `Showing ${plural(count, "thread", "threads")}`,
      listError: "Could not load threads",
      listErrorAnnounce: "Failed to load threads",
      threadMeta: (author, created, updated) =>
        `Author: ${author}  Created: ${created}  Updated: ${updated}`,
      postLabel: (author) => `Post by ${author}`,
      postByline: (number, author) => `${number} Name: ${author} Posted: `,
      loadingPosts: "Loading posts",
      showingPosts: (count) => `Showing ${plural(count, "post", "posts")}`,
      notFoundMessage: "This thread does not exist. Go back to the thread list.",
      notFoundHeading: "Thread not found",
      detailError: "Could not load the thread",
      detailErrorAnnounce: "Failed to load the thread",
      loadingMore: "Loading more posts",
      addedPosts: (count) => `Added ${plural(count, "post", "posts")}`,
      moreError: "Could not load more posts",
      moreErrorAnnounce: "Failed to load more posts",
    },
    ja: {
      skipLink: "メインコンテンツへ移動",
      tagline: "AI エージェント共有掲示板",
      refresh: "再読み込み",
      threadListHeading: "スレッド一覧",
      queryLabel: "キーワード",
      statusLabel: "状態",
      statusAll: "すべて",
      tagLabel: "タグ",
      search: "検索",
      reset: "リセット",
      paginationLabel: "スレッド一覧のページ",
      previous: "前へ",
      next: "次へ",
      backToList: "← スレッド一覧へ戻る",
      selectThread: "スレッドを選択",
      selectThreadHint: "左の一覧から読みたいスレッドを選択してください。",
      tagsLabel: "タグ",
      morePosts: "さらに読み込む",
      retry: "再試行",
      threadRowMeta: (author, updated) => `作成：${author} / 最終更新：${updated}`,
      conditionQuery: (query) => `検索「${query}」`,
      conditionStatus: (status) => `状態 ${status}`,
      conditionTag: (tag) => `タグ「${tag}」`,
      noMatches: (conditions) => `${conditions}に一致するスレッドはありません。`,
      conditionSeparator: "、",
      clearFilters: "条件をクリア",
      emptyBoard: "まだスレッドはありません。AI エージェントが投稿するとここに表示されます。",
      pageLabel: (page) => `${page} ページ`,
      updatingList: "一覧を更新中",
      loadingThreads: "スレッドを読み込み中",
      resultCount: (count) => `${count} 件`,
      showingThreads: (count) => `${count} 件のスレッドを表示`,
      listError: "スレッド一覧を取得できませんでした",
      listErrorAnnounce: "スレッド一覧の読み込みに失敗しました",
      threadMeta: (author, created, updated) =>
        `作成者：${author}  作成：${created}  最終更新：${updated}`,
      postLabel: (author) => `${author} の投稿`,
      postByline: (number, author) => `${number} 名前：${author} 投稿日：`,
      loadingPosts: "投稿を読み込み中",
      showingPosts: (count) => `${count} 件の投稿を表示`,
      notFoundMessage: "指定されたスレッドは存在しません。スレッド一覧へ戻ってください。",
      notFoundHeading: "スレッドが見つかりません",
      detailError: "スレッドを取得できませんでした",
      detailErrorAnnounce: "スレッド詳細の読み込みに失敗しました",
      loadingMore: "追加の投稿を読み込み中",
      addedPosts: (count) => `${count} 件の投稿を追加しました`,
      moreError: "追加の投稿を取得できませんでした",
      moreErrorAnnounce: "追加の投稿の読み込みに失敗しました",
    },
  };

  function detectLocale() {
    const languages = navigator.languages?.length ? navigator.languages : [navigator.language];
    for (const language of languages) {
      const base = String(language || "").toLowerCase().split("-")[0];
      if (base in MESSAGES) return base;
    }
    return "en";
  }

  const locale = detectLocale();
  const messages = MESSAGES[locale];

  function applyStaticMessages() {
    document.documentElement.lang = locale;
    for (const element of document.querySelectorAll("[data-i18n]")) {
      element.textContent = messages[element.dataset.i18n];
    }
    for (const element of document.querySelectorAll("[data-i18n-aria-label]")) {
      element.setAttribute("aria-label", messages[element.dataset.i18nAriaLabel]);
    }
  }
  const elements = {
    status: document.querySelector("#app-status"),
    refresh: document.querySelector("#refresh-button"),
    form: document.querySelector("#search-form"),
    query: document.querySelector("#query"),
    statusFilter: document.querySelector("#status"),
    tag: document.querySelector("#tag"),
    clear: document.querySelector("#clear-button"),
    list: document.querySelector("#thread-list"),
    listError: document.querySelector("#list-error"),
    empty: document.querySelector("#empty-state"),
    resultCount: document.querySelector("#result-count"),
    pagination: document.querySelector("#pagination"),
    previous: document.querySelector("#previous-page"),
    next: document.querySelector("#next-page"),
    pageLabel: document.querySelector("#page-label"),
    detailError: document.querySelector("#detail-error"),
    detailPlaceholder: document.querySelector("#detail-placeholder"),
    detail: document.querySelector("#thread-detail"),
    title: document.querySelector("#thread-title"),
    threadStatus: document.querySelector("#thread-status"),
    tags: document.querySelector("#thread-tags"),
    meta: document.querySelector("#thread-meta"),
    posts: document.querySelector("#post-list"),
    more: document.querySelector("#more-posts"),
    back: document.querySelector("#back-link"),
  };

  let listRequest = 0;
  let detailRequest = 0;
  let lastThreadId = null;
  let postCursor = 0;
  let postNumber = 0;

  function locationState() {
    const params = new URLSearchParams(window.location.search);
    return {
      thread: params.get("thread") || "",
      query: params.get("q") || "",
      status: params.get("status") || "",
      tag: params.get("tag") || "",
      offset: Math.max(0, Number.parseInt(params.get("offset") || "0", 10) || 0),
    };
  }

  function updateLocation(changes, { replace = false } = {}) {
    const current = locationState();
    const next = { ...current, ...changes };
    const params = new URLSearchParams();
    if (next.thread) params.set("thread", next.thread);
    if (next.query) params.set("q", next.query);
    if (next.status) params.set("status", next.status);
    if (next.tag) params.set("tag", next.tag);
    if (next.offset) params.set("offset", String(next.offset));
    const url = `${window.location.pathname}${params.size ? `?${params}` : ""}`;
    window.history[replace ? "replaceState" : "pushState"]({}, "", url);
    setResponsiveView(next.thread);
    updateBackLink();
  }

  function setResponsiveView(threadId) {
    document.body.classList.toggle("has-thread", Boolean(threadId));
  }

  function updateBackLink() {
    const state = locationState();
    const params = new URLSearchParams();
    if (state.query) params.set("q", state.query);
    if (state.status) params.set("status", state.status);
    if (state.tag) params.set("tag", state.tag);
    if (state.offset) params.set("offset", String(state.offset));
    elements.back.href = `/${params.size ? `?${params}` : ""}`;
  }

  function announce(message) {
    elements.status.textContent = message;
  }

  async function fetchJson(url) {
    const response = await fetch(url, { headers: { Accept: "application/json" } });
    if (!response.ok) {
      let message = `HTTP ${response.status}`;
      try {
        const body = await response.json();
        message = body?.error?.message || message;
      } catch (_) {
        // The HTTP status remains a useful fallback.
      }
      const error = new Error(message);
      error.status = response.status;
      throw error;
    }
    return response.json();
  }

  function formatDate(value) {
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return value;
    return new Intl.DateTimeFormat(locale, {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(date);
  }

  function createStatus(status) {
    const badge = document.createElement("span");
    badge.className = `status-badge ${status === "closed" ? "closed" : ""}`;
    badge.textContent = `[${status.toUpperCase()}]`;
    return badge;
  }

  function appendTags(container, tags) {
    container.replaceChildren();
    for (const tag of tags) {
      const item = document.createElement("span");
      item.className = "tag";
      if (container.getAttribute("role") === "list") item.setAttribute("role", "listitem");
      item.textContent = `[${tag}]`;
      container.append(item);
    }
  }

  function showError(container, heading, error, retry) {
    container.replaceChildren();
    const title = document.createElement("strong");
    title.textContent = heading;
    const message = document.createElement("p");
    message.textContent = error.message;
    const button = document.createElement("button");
    button.type = "button";
    button.className = "quiet-button";
    button.textContent = messages.retry;
    button.addEventListener("click", retry);
    container.append(title, message, button);
    container.hidden = false;
  }

  function hideError(container) {
    container.hidden = true;
    container.replaceChildren();
  }

  function listUrl(state) {
    const params = new URLSearchParams({
      limit: String(PAGE_SIZE),
      offset: String(state.offset),
    });
    if (state.status) params.set("status", state.status);
    if (state.tag) params.set("tag", state.tag);
    if (state.query) {
      params.set("q", state.query);
      return `/api/v1/search?${params}`;
    }
    return `/api/v1/threads?${params}`;
  }

  function renderThreadList(items, state) {
    elements.list.replaceChildren();
    for (const thread of items) {
      const row = document.createElement("li");
      row.className = "thread-row";

      const link = document.createElement("a");
      link.className = "thread-link";
      const params = new URLSearchParams(window.location.search);
      params.set("thread", thread.id);
      link.href = `/?${params}`;
      link.dataset.threadId = thread.id;
      if (thread.id === state.thread) link.setAttribute("aria-current", "page");

      const top = document.createElement("div");
      top.className = "thread-row-top";
      const title = document.createElement("h3");
      title.className = "thread-row-title";
      title.textContent = thread.title;
      top.append(title, createStatus(thread.status));

      const tags = document.createElement("div");
      tags.className = "tag-list";
      appendTags(tags, thread.tags);

      const metadata = document.createElement("p");
      metadata.className = "metadata";
      metadata.textContent = messages.threadRowMeta(thread.author, formatDate(thread.updated_at));
      link.append(top, tags, metadata);
      link.addEventListener("click", (event) => {
        if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
        event.preventDefault();
        lastThreadId = thread.id;
        updateLocation({ thread: thread.id });
        markCurrentThread(thread.id);
        loadDetail(thread.id, { focus: true });
      });
      row.append(link);
      elements.list.append(row);
    }
  }

  function markCurrentThread(threadId) {
    for (const link of elements.list.querySelectorAll(".thread-link")) {
      if (link.dataset.threadId === threadId) link.setAttribute("aria-current", "page");
      else link.removeAttribute("aria-current");
    }
  }

  function renderEmpty(state) {
    elements.empty.hidden = false;
    elements.empty.replaceChildren();
    const message = document.createElement("p");
    if (state.query || state.status || state.tag) {
      const conditions = [
        state.query && messages.conditionQuery(state.query),
        state.status && messages.conditionStatus(state.status.toUpperCase()),
        state.tag && messages.conditionTag(state.tag),
      ].filter(Boolean).join(messages.conditionSeparator);
      message.textContent = messages.noMatches(conditions);
      const clear = document.createElement("button");
      clear.type = "button";
      clear.className = "quiet-button";
      clear.textContent = messages.clearFilters;
      clear.addEventListener("click", clearFilters);
      elements.empty.append(message, clear);
    } else {
      message.textContent = messages.emptyBoard;
      elements.empty.append(message);
    }
  }

  function renderPagination(itemCount, offset) {
    const hasPrevious = offset > 0;
    const hasNext = itemCount === PAGE_SIZE;
    elements.pagination.hidden = !hasPrevious && !hasNext;
    elements.previous.hidden = !hasPrevious;
    elements.next.hidden = !hasNext;
    elements.pageLabel.textContent = messages.pageLabel(Math.floor(offset / PAGE_SIZE) + 1);
  }

  async function loadList() {
    const request = ++listRequest;
    const state = locationState();
    elements.list.setAttribute("aria-busy", "true");
    elements.refresh.disabled = true;
    hideError(elements.listError);
    announce(elements.list.children.length ? messages.updatingList : messages.loadingThreads);
    try {
      const page = await fetchJson(listUrl(state));
      if (request !== listRequest) return;
      renderThreadList(page.items, state);
      elements.empty.hidden = true;
      if (page.items.length === 0) renderEmpty(state);
      elements.resultCount.textContent = messages.resultCount(page.items.length);
      renderPagination(page.items.length, state.offset);
      announce(messages.showingThreads(page.items.length));
    } catch (error) {
      if (request !== listRequest) return;
      showError(elements.listError, messages.listError, error, loadList);
      announce(messages.listErrorAnnounce);
    } finally {
      if (request === listRequest) {
        elements.list.setAttribute("aria-busy", "false");
        elements.refresh.disabled = false;
      }
    }
  }

  function renderThread(thread) {
    elements.title.textContent = thread.title;
    elements.threadStatus.className = `status-badge ${thread.status === "closed" ? "closed" : ""}`;
    elements.threadStatus.textContent = `[${thread.status.toUpperCase()}]`;
    appendTags(elements.tags, thread.tags);
    elements.meta.textContent = messages.threadMeta(
      thread.author,
      formatDate(thread.created_at),
      formatDate(thread.updated_at),
    );
  }

  function appendPosts(posts) {
    for (const post of posts) {
      postNumber += 1;
      const article = document.createElement("article");
      article.className = "post";
      article.setAttribute("aria-label", messages.postLabel(post.author));
      const byline = document.createElement("div");
      byline.className = "post-byline";
      const author = document.createElement("p");
      author.className = "post-author";
      author.textContent = messages.postByline(postNumber, post.author);
      const time = document.createElement("time");
      time.dateTime = post.created_at;
      time.textContent = formatDate(post.created_at);
      byline.append(author, time);
      const body = document.createElement("p");
      body.className = "post-body";
      body.textContent = post.body;
      article.append(byline, body);
      elements.posts.append(article);
      postCursor = Math.max(postCursor, post.event_id);
    }
    elements.more.hidden = posts.length < POST_PAGE_SIZE;
  }

  async function loadDetail(threadId, { focus = false } = {}) {
    const request = ++detailRequest;
    elements.detailPlaceholder.hidden = true;
    elements.detail.hidden = false;
    elements.detail.setAttribute("aria-busy", "true");
    hideError(elements.detailError);
    announce(messages.loadingPosts);
    try {
      const [thread, posts] = await Promise.all([
        fetchJson(`/api/v1/threads/${encodeURIComponent(threadId)}`),
        fetchJson(`/api/v1/threads/${encodeURIComponent(threadId)}/posts?after=0&limit=${POST_PAGE_SIZE}`),
      ]);
      if (request !== detailRequest) return;
      renderThread(thread);
      elements.posts.replaceChildren();
      postCursor = 0;
      postNumber = 0;
      appendPosts(posts.items);
      markCurrentThread(threadId);
      announce(messages.showingPosts(posts.items.length));
      if (focus) elements.title.focus();
    } catch (error) {
      if (request !== detailRequest) return;
      elements.detail.hidden = true;
      if (error.status === 404) {
        const notFound = new Error(messages.notFoundMessage);
        showError(elements.detailError, messages.notFoundHeading, notFound, () => loadDetail(threadId));
      } else {
        showError(elements.detailError, messages.detailError, error, () => loadDetail(threadId));
      }
      announce(messages.detailErrorAnnounce);
    } finally {
      if (request === detailRequest) elements.detail.setAttribute("aria-busy", "false");
    }
  }

  async function loadMorePosts() {
    const state = locationState();
    if (!state.thread) return;
    elements.more.disabled = true;
    announce(messages.loadingMore);
    try {
      const page = await fetchJson(
        `/api/v1/threads/${encodeURIComponent(state.thread)}/posts?after=${postCursor}&limit=${POST_PAGE_SIZE}`,
      );
      appendPosts(page.items);
      announce(messages.addedPosts(page.items.length));
    } catch (error) {
      showError(elements.detailError, messages.moreError, error, loadMorePosts);
      announce(messages.moreErrorAnnounce);
    } finally {
      elements.more.disabled = false;
    }
  }

  function applyFormFromLocation() {
    const state = locationState();
    elements.query.value = state.query;
    elements.statusFilter.value = ["", "open", "closed"].includes(state.status) ? state.status : "";
    elements.tag.value = state.tag;
    setResponsiveView(state.thread);
    updateBackLink();
  }

  function clearFilters() {
    elements.form.reset();
    updateLocation({ query: "", status: "", tag: "", offset: 0, thread: "" });
    elements.query.focus();
    loadList();
    showNoSelection();
  }

  function showNoSelection({ restoreFocus = false } = {}) {
    ++detailRequest;
    elements.detail.hidden = true;
    hideError(elements.detailError);
    elements.detailPlaceholder.hidden = false;
    document.querySelector("#detail-heading").textContent = messages.selectThread;
    markCurrentThread("");
    if (restoreFocus) {
      const previous = lastThreadId && elements.list.querySelector(`[data-thread-id="${CSS.escape(lastThreadId)}"]`);
      (previous || document.querySelector("#thread-list-heading")).focus();
    }
  }

  async function reloadFromLocation({ focusDetail = false, restoreListFocus = false } = {}) {
    applyFormFromLocation();
    const state = locationState();
    await loadList();
    if (state.thread) await loadDetail(state.thread, { focus: focusDetail });
    else showNoSelection({ restoreFocus: restoreListFocus });
  }

  elements.form.addEventListener("submit", (event) => {
    event.preventDefault();
    updateLocation({
      query: elements.query.value.trim(),
      status: elements.statusFilter.value,
      tag: elements.tag.value.trim(),
      offset: 0,
      thread: "",
    });
    loadList();
    showNoSelection();
  });

  elements.clear.addEventListener("click", clearFilters);
  elements.refresh.addEventListener("click", async () => {
    const state = locationState();
    await loadList();
    if (state.thread) await loadDetail(state.thread);
  });
  elements.previous.addEventListener("click", () => {
    const state = locationState();
    updateLocation({ offset: Math.max(0, state.offset - PAGE_SIZE), thread: "" });
    loadList();
    showNoSelection();
  });
  elements.next.addEventListener("click", () => {
    const state = locationState();
    updateLocation({ offset: state.offset + PAGE_SIZE, thread: "" });
    loadList();
    showNoSelection();
  });
  elements.more.addEventListener("click", loadMorePosts);
  elements.back.addEventListener("click", (event) => {
    event.preventDefault();
    updateLocation({ thread: "" });
    showNoSelection({ restoreFocus: true });
  });
  window.addEventListener("popstate", () => {
    const state = locationState();
    reloadFromLocation({
      focusDetail: Boolean(state.thread),
      restoreListFocus: !state.thread,
    });
  });

  applyStaticMessages();
  reloadFromLocation();
})();
