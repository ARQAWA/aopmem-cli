"use strict";

(() => {
  const API_BASE = new URL("./", window.location.href);
  const PAGE_LIMIT = 100;
  const MAX_PAGE_SIZE = 500;
  const MAX_GRAPH_NODES = 200;
  const MAX_GRAPH_EDGES = 500;
  const MAX_SEARCH_BYTES = 512;
  const SVG_NAMESPACE = "http:" + "//www.w3.org/2000/svg";

  const VIEW_META = Object.freeze({
    overview: {
      title: "Overview",
      kicker: "Workspace status",
      summary: "Health, inventory, and the latest local recall evidence.",
    },
    memory: {
      title: "Memory",
      kicker: "Operational memory",
      summary: "Search bounded node pages and inspect complete node details.",
    },
    graph: {
      title: "Graph",
      kicker: "Selected subgraph",
      summary: "A deterministic, bounded view of nodes and their links.",
    },
    activity: {
      title: "Activity",
      kicker: "Local observability",
      summary: "Fact-only command events and recall bundle evidence.",
    },
    effectiveness: {
      title: "Effectiveness",
      kicker: "Measured facts",
      summary: "Verifiable recall, feedback, tool, and health data. No score.",
    },
    tools: {
      title: "Tools / MCP",
      kicker: "Execution contracts",
      summary: "Read-only status, side effects, and approval requirements.",
    },
  });

  const NODE_TYPES = Object.freeze([
    "kernel_contract",
    "gate",
    "rule",
    "workflow",
    "skill",
    "tool_contract",
    "mcp_profile",
    "project_profile",
    "project_fact",
    "decision",
    "correction",
    "lesson",
    "failure_mode",
    "incident_scar",
    "preference",
    "reflection_observation",
    "raw_note",
    "hunch_source",
    "source",
  ]);

  const NODE_STATUSES = Object.freeze([
    "draft",
    "active",
    "deprecated",
    "superseded",
    "broken",
  ]);

  const TOOL_SIDE_EFFECTS = Object.freeze([
    "none",
    "local_read",
    "local_write_artifact",
    "local_write_memory",
    "external_read",
    "external_write",
    "destructive",
  ]);

  const MCP_STATUSES = Object.freeze([
    "installed",
    "missing",
    "configured_unverified",
  ]);

  const EVENT_TYPES = Object.freeze([
    "install.started",
    "install.completed",
    "install.failed",
    "update.started",
    "update.completed",
    "update.failed",
    "workspace.init",
    "adapter.seed",
    "adapter.sync",
    "adapter.drift",
    "recall.started",
    "recall.completed",
    "recall.failed",
    "recall.continuation",
    "recall.empty",
    "recall.truncated",
    "recall.mandatory_overflow",
    "node.created",
    "node.updated",
    "node.deprecated",
    "link.created",
    "remember",
    "teach.started",
    "teach.proposed",
    "teach.applied",
    "reflection.inventory",
    "reflection.proposal",
    "reflection.applied",
    "tool.validation",
    "tool.run.started",
    "tool.run.completed",
    "tool.run.failed",
    "tool.run.timeout",
    "tool.output.artifact",
    "mcp.status",
    "doctor",
    "verify",
    "audit.snapshot.completed",
    "audit.snapshot.pending",
    "audit.snapshot.failed",
    "artifacts.cleanup",
    "feedback.recorded",
  ]);

  const EVENT_OUTCOMES = Object.freeze([
    "started",
    "success",
    "failure",
    "warning",
    "empty",
    "truncated",
    "overflow",
    "pending",
    "blocked",
    "timeout",
    "recorded",
    "proposed",
    "applied",
    "drafted",
    "missing",
    "configured",
    "configured_unverified",
  ]);

  const BADGE_CLASS_BY_VALUE = new Map([
    ["active", "badge-success"],
    ["success", "badge-success"],
    ["installed", "badge-success"],
    ["configured", "badge-success"],
    ["useful", "badge-success"],
    ["applied", "badge-success"],
    ["recorded", "badge-success"],
    ["ready", "badge-success"],
    ["completed", "badge-success"],
    ["draft", "badge-warning"],
    ["warning", "badge-warning"],
    ["pending", "badge-warning"],
    ["partial", "badge-warning"],
    ["configured_unverified", "badge-warning"],
    ["proposed", "badge-warning"],
    ["drafted", "badge-warning"],
    ["started", "badge-warning"],
    ["truncated", "badge-warning"],
    ["broken", "badge-danger"],
    ["failure", "badge-danger"],
    ["failed", "badge-danger"],
    ["missing", "badge-danger"],
    ["wrong", "badge-danger"],
    ["timeout", "badge-danger"],
    ["overflow", "badge-danger"],
    ["blocked", "badge-danger"],
    ["deprecated", "badge-accent"],
    ["superseded", "badge-accent"],
  ]);

  const elements = Object.freeze({
    workspaceName: document.getElementById("workspace-name"),
    productVersion: document.getElementById("product-version"),
    refreshView: document.getElementById("refresh-view"),
    primaryNav: document.getElementById("primary-nav"),
    main: document.getElementById("main-content"),
    viewKicker: document.getElementById("view-kicker"),
    viewTitle: document.getElementById("view-title"),
    viewSummary: document.getElementById("view-summary"),
    viewContent: document.getElementById("view-content"),
    detailPanel: document.getElementById("detail-panel"),
    detailTitle: document.getElementById("detail-title"),
    detailContent: document.getElementById("detail-content"),
    closeDetail: document.getElementById("close-detail"),
    liveStatus: document.getElementById("live-status"),
  });

  function newPageState() {
    return {
      cursor: null,
      back: [],
      next: null,
      moreResults: false,
      itemCount: 0,
    };
  }

  function newRequestSlot() {
    return {
      controller: null,
      sequence: 0,
    };
  }

  const state = {
    activeView: "overview",
    bootstrap: null,
    bootRequest: newRequestSlot(),
    viewRequest: newRequestSlot(),
    detailRequest: newRequestSlot(),
    detailReturnFocus: null,
    filters: {
      memory: { nodeType: "", status: "", search: "" },
      graph: { nodeType: "", status: "", center: "" },
      activity: { eventType: "", outcome: "", command: "" },
      tools: { status: "", sideEffects: "" },
      mcp: { status: "", kind: "" },
    },
    pages: {
      memory: newPageState(),
      graph: newPageState(),
      activity: newPageState(),
      tools: newPageState(),
      mcp: newPageState(),
      nodeLinks: newPageState(),
      bundle: newPageState(),
    },
    nodeDetailId: null,
    bundleId: null,
  };

  class UiRequestError extends Error {
    constructor(code, message) {
      super(message);
      this.name = "UiRequestError";
      this.code = code;
    }
  }

  function makeElement(tagName, className, textValue) {
    const result = document.createElement(tagName);
    if (className) {
      result.className = className;
    }
    if (textValue !== undefined && textValue !== null) {
      result.textContent = String(textValue);
    }
    return result;
  }

  function makeSvgElement(tagName, className) {
    const result = document.createElementNS(SVG_NAMESPACE, tagName);
    if (className) {
      result.setAttribute("class", className);
    }
    return result;
  }

  function makeButton(label, handler, className = "button") {
    const result = makeElement("button", className, label);
    result.type = "button";
    result.addEventListener("click", handler);
    return result;
  }

  function makeSubmitButton(label) {
    const result = makeElement("button", "button button-primary", label);
    result.type = "submit";
    return result;
  }

  function isRecord(value) {
    return value !== null && typeof value === "object" && !Array.isArray(value);
  }

  function asArray(value) {
    return Array.isArray(value) ? value : [];
  }

  function displayValue(value, fallback = "—") {
    if (value === null || value === undefined || value === "") {
      return fallback;
    }
    if (typeof value === "boolean") {
      return value ? "Yes" : "No";
    }
    return String(value);
  }

  function displayNumber(value, fallback = "0") {
    return typeof value === "number" && Number.isFinite(value)
      ? String(value)
      : fallback;
  }

  function displayDecimal(value, digits = 2) {
    return typeof value === "number" && Number.isFinite(value)
      ? value.toFixed(digits)
      : "—";
  }

  function displayDuration(value) {
    return typeof value === "number" && Number.isFinite(value)
      ? `${value} ms`
      : "—";
  }

  function truncateLabel(value, maximum = 22) {
    const characters = Array.from(displayValue(value, "Untitled"));
    if (characters.length <= maximum) {
      return characters.join("");
    }
    return `${characters.slice(0, maximum - 1).join("")}…`;
  }

  function safeErrorCode(value) {
    return typeof value === "string" && /^[A-Za-z0-9_.:-]{1,128}$/.test(value)
      ? value
      : "UI_REQUEST_FAILED";
  }

  function safeErrorMessage(value) {
    return typeof value === "string" && value.length > 0 && value.length <= 240
      && !/[\u0000-\u001f\u007f]/.test(value)
      ? value
      : "The local UI request failed.";
  }

  function announce(message) {
    elements.liveStatus.textContent = "";
    window.setTimeout(() => {
      elements.liveStatus.textContent = message;
    }, 0);
  }

  function beginRequest(slot) {
    if (slot.controller) {
      slot.controller.abort();
    }
    slot.controller = new AbortController();
    slot.sequence += 1;
    return {
      signal: slot.controller.signal,
      sequence: slot.sequence,
    };
  }

  function requestIsCurrent(slot, request) {
    return slot.sequence === request.sequence && !request.signal.aborted;
  }

  function isAbortError(error) {
    return isRecord(error) && error.name === "AbortError";
  }

  async function requestJson(endpoint, parameters, signal) {
    const url = new URL(`api/v1/${endpoint}`, API_BASE);
    for (const [name, value] of Object.entries(parameters || {})) {
      if (value !== null && value !== undefined && value !== "") {
        url.searchParams.set(name, String(value));
      }
    }
    if (url.origin !== window.location.origin
        || !url.pathname.startsWith(API_BASE.pathname)) {
      throw new UiRequestError("UI_ROUTE_REJECTED", "The local API route was rejected.");
    }

    let response;
    try {
      response = await window.fetch(url, {
        credentials: "omit",
        cache: "no-store",
        redirect: "error",
        referrerPolicy: "no-referrer",
        headers: { Accept: "application/json" },
        signal,
      });
    } catch (error) {
      if (isAbortError(error)) {
        throw error;
      }
      throw new UiRequestError(
        "UI_CONNECTION_FAILED",
        "The local AOPMem UI server is unavailable.",
      );
    }

    let body;
    try {
      body = await response.json();
    } catch (_error) {
      throw new UiRequestError(
        "UI_RESPONSE_INVALID",
        "The local UI returned an invalid response.",
      );
    }

    if (!response.ok) {
      const detail = isRecord(body) && isRecord(body.error) ? body.error : {};
      throw new UiRequestError(
        safeErrorCode(detail.code),
        safeErrorMessage(detail.message),
      );
    }
    if (!isRecord(body)) {
      throw new UiRequestError(
        "UI_RESPONSE_INVALID",
        "The local UI returned an invalid response.",
      );
    }
    return body;
  }

  function setViewState(viewState, busy = false) {
    elements.viewContent.dataset.viewState = viewState;
    elements.viewContent.setAttribute("aria-busy", busy ? "true" : "false");
  }

  function renderLoading(target = elements.viewContent, label = "Loading local data…") {
    const wrapper = makeElement("div", "loading-state");
    wrapper.append(
      makeElement("strong", "", label),
      makeElement("span", "", "Read-only request in progress."),
    );
    target.replaceChildren(wrapper);
    if (target === elements.viewContent) {
      setViewState("loading", true);
    } else {
      target.setAttribute("aria-busy", "true");
    }
  }

  function renderError(error, retry, target = elements.viewContent) {
    const code = safeErrorCode(error && error.code);
    const message = safeErrorMessage(error && error.message);
    const wrapper = makeElement("div", "error-state");
    wrapper.append(
      makeElement("strong", "", code),
      makeElement("span", "", message),
    );
    if (typeof retry === "function") {
      wrapper.append(makeButton("Retry", retry, "button button-primary"));
    }
    target.replaceChildren(wrapper);
    if (target === elements.viewContent) {
      setViewState("error", false);
    } else {
      target.setAttribute("aria-busy", "false");
    }
    announce(`${code}. ${message}`);
  }

  function makeNotice(message, kind = "partial") {
    const className = kind === "info"
      ? "info-notice"
      : kind === "danger"
        ? "danger-notice"
        : "partial-notice";
    const result = makeElement("p", className, message);
    result.setAttribute("role", kind === "danger" ? "alert" : "status");
    return result;
  }

  function makeBadge(value) {
    const normalized = displayValue(value, "unknown").toLowerCase();
    const badgeClass = BADGE_CLASS_BY_VALUE.get(normalized);
    const className = badgeClass ? `badge ${badgeClass}` : "badge";
    return makeElement("span", className, normalized.replaceAll("_", " "));
  }

  function makeMetricStrip(metrics) {
    const strip = makeElement("div", "metric-strip");
    for (const metric of metrics) {
      const item = makeElement("div", "metric");
      item.append(
        makeElement("span", "metric-label", metric.label),
        makeElement("span", "metric-value", metric.value),
      );
      if (metric.note) {
        item.append(makeElement("span", "metric-note", metric.note));
      }
      strip.append(item);
    }
    return strip;
  }

  function makeSection(title, note) {
    const section = makeElement("section", "section-block");
    const heading = makeElement("div", "section-title-row");
    heading.append(makeElement("h2", "", title));
    if (note) {
      heading.append(makeElement("p", "section-note", note));
    }
    section.append(heading);
    return section;
  }

  function makeDefinitionList(items, className = "detail-list") {
    const list = makeElement("dl", className);
    for (const item of items) {
      const row = makeElement("div", "detail-row");
      const term = makeElement("dt", "", item.label);
      const description = makeElement("dd");
      if (item.node instanceof Node) {
        description.append(item.node);
      } else {
        description.textContent = displayValue(item.value);
      }
      row.append(term, description);
      list.append(row);
    }
    return list;
  }

  function makeTable(captionText, headings, className = "data-table") {
    const wrapper = makeElement("div", "table-wrap");
    const table = makeElement("table", className);
    const caption = makeElement("caption", "visually-hidden", captionText);
    const head = makeElement("thead");
    const headRow = makeElement("tr");
    for (const headingText of headings) {
      const heading = makeElement("th", "", headingText);
      heading.scope = "col";
      headRow.append(heading);
    }
    head.append(headRow);
    const body = makeElement("tbody");
    table.append(caption, head, body);
    wrapper.append(table);
    return { wrapper, body };
  }

  function appendCell(row, value, className) {
    row.append(makeElement("td", className || "", displayValue(value)));
  }

  function makeSelectField(id, label, values, current, emptyLabel) {
    const wrapper = makeElement("div", "filter-field");
    const labelElement = makeElement("label", "", label);
    labelElement.htmlFor = id;
    const select = makeElement("select");
    select.id = id;
    const empty = makeElement("option", "", emptyLabel);
    empty.value = "";
    select.append(empty);
    for (const value of values) {
      const option = makeElement("option", "", value.replaceAll("_", " "));
      option.value = value;
      select.append(option);
    }
    select.value = current;
    wrapper.append(labelElement, select);
    return { wrapper, select };
  }

  function makeInputField(id, label, current, placeholder, wide = false) {
    const wrapper = makeElement(
      "div",
      wide ? "filter-field filter-field-wide" : "filter-field",
    );
    const labelElement = makeElement("label", "", label);
    labelElement.htmlFor = id;
    const input = makeElement("input");
    input.id = id;
    input.type = "text";
    input.value = current;
    input.placeholder = placeholder;
    input.autocomplete = "off";
    input.spellcheck = false;
    wrapper.append(labelElement, input);
    return { wrapper, input };
  }

  function resetPage(page) {
    page.cursor = null;
    page.back = [];
    page.next = null;
    page.moreResults = false;
    page.itemCount = 0;
  }

  function updatePage(page, response, nextField = "next_cursor", moreField = "more_results") {
    const moreResults = response[moreField] === true;
    const nextCursor = response[nextField];
    if (moreResults && (typeof nextCursor !== "string" || nextCursor.length === 0)) {
      throw new UiRequestError(
        "UI_RESPONSE_INVALID",
        "An incomplete page did not provide a continuation cursor.",
      );
    }
    page.next = typeof nextCursor === "string" ? nextCursor : null;
    page.moreResults = moreResults;
  }

  function goToNextPage(page, reload) {
    if (!page.next) {
      return;
    }
    page.back.push(page.cursor);
    page.cursor = page.next;
    page.next = null;
    reload();
  }

  function goToPreviousPage(page, reload) {
    if (page.back.length === 0) {
      return;
    }
    page.cursor = page.back.pop() || null;
    page.next = null;
    reload();
  }

  function makePagination(label, page, reload, moreMessage = "More results are available.") {
    const wrapper = makeElement("div", "pagination");
    const status = `${label} ${page.back.length + 1} · ${page.itemCount} shown`
      + (page.moreResults ? ` · ${moreMessage}` : " · End of results");
    wrapper.append(makeElement("span", "pagination-status", status));
    const actions = makeElement("div", "pagination-actions");
    const previous = makeButton(
      "Previous",
      () => goToPreviousPage(page, reload),
      "button button-quiet",
    );
    previous.disabled = page.back.length === 0;
    const next = makeButton(
      "Next",
      () => goToNextPage(page, reload),
      "button button-quiet",
    );
    next.disabled = !page.next;
    actions.append(previous, next);
    wrapper.append(actions);
    return wrapper;
  }

  function validateBoundedItems(response, field, maximum) {
    const items = asArray(response[field]);
    if (items.length > maximum) {
      throw new UiRequestError(
        "UI_RESPONSE_TOO_LARGE",
        "The local UI returned more items than the contract allows.",
      );
    }
    return items;
  }

  function closeDetail(restoreFocus = true) {
    if (state.detailRequest.controller) {
      state.detailRequest.controller.abort();
    }
    elements.detailPanel.hidden = true;
    elements.detailContent.replaceChildren();
    elements.detailContent.setAttribute("aria-busy", "false");
    if (restoreFocus && state.detailReturnFocus instanceof HTMLElement
        && document.contains(state.detailReturnFocus)) {
      state.detailReturnFocus.focus();
    }
    state.detailReturnFocus = null;
  }

  function openDetail(title, trigger) {
    if (trigger instanceof HTMLElement && !elements.detailPanel.contains(trigger)) {
      state.detailReturnFocus = trigger;
    }
    elements.detailTitle.textContent = title;
    elements.detailTitle.tabIndex = -1;
    elements.detailPanel.hidden = false;
    renderLoading(elements.detailContent, "Loading details…");
  }

  function updateNavigation(view) {
    for (const item of elements.primaryNav.querySelectorAll("[data-view]")) {
      if (item.dataset.view === view) {
        item.setAttribute("aria-current", "page");
      } else {
        item.removeAttribute("aria-current");
      }
    }
    const meta = VIEW_META[view];
    elements.viewTitle.textContent = meta.title;
    elements.viewKicker.textContent = meta.kicker;
    elements.viewSummary.textContent = meta.summary;
    elements.main.dataset.activeView = view;
  }

  async function showView(view, focusHeading = true) {
    if (!Object.hasOwn(VIEW_META, view)) {
      return;
    }
    state.activeView = view;
    closeDetail(false);
    updateNavigation(view);
    if (focusHeading) {
      elements.viewTitle.focus();
    }
    renderLoading();
    const request = beginRequest(state.viewRequest);
    try {
      await VIEW_LOADERS[view](request);
    } catch (error) {
      if (!isAbortError(error) && requestIsCurrent(state.viewRequest, request)) {
        renderError(error, () => showView(view, false));
      }
    }
  }

  function setReadyState(isEmpty = false, isPartial = false) {
    setViewState(isPartial ? "partial" : isEmpty ? "empty" : "ready", false);
  }

  async function loadOverview(request) {
    const response = await requestJson("overview", {}, request.signal);
    if (!requestIsCurrent(state.viewRequest, request)) {
      return;
    }
    renderOverview(response);
  }

  function renderOverview(response) {
    const memory = isRecord(response.memory) ? response.memory : {};
    const observability = isRecord(response.observability) ? response.observability : {};
    const nodeCount = displayNumber(memory.node_count);
    const linkCount = displayNumber(memory.link_count);
    const metricStrip = makeMetricStrip([
      { label: "Nodes", value: nodeCount, note: `${displayNumber(memory.draft)} draft` },
      { label: "Links", value: linkCount, note: `${displayNumber(memory.orphaned)} orphaned nodes` },
      { label: "Tools", value: displayNumber(response.tool_count), note: "Registered contracts" },
      { label: "MCP", value: displayNumber(response.mcp_count), note: "Local profiles" },
      { label: "Broken", value: displayNumber(memory.broken), note: "Memory status" },
      { label: "Deprecated", value: displayNumber(memory.deprecated), note: "Still inspectable" },
    ]);

    const content = document.createDocumentFragment();
    content.append(metricStrip);
    if (observability.collection_status === "not_collected") {
      content.append(makeNotice(
        "Local Observability has not been collected for this workspace yet.",
        "info",
      ));
    }

    const columns = makeElement("div", "section-grid");
    const healthSection = makeSection("Health", "Latest recorded checks");
    const health = isRecord(observability.health) ? observability.health : {};
    const doctor = isRecord(health.doctor) ? health.doctor : {};
    const verify = isRecord(health.verify) ? health.verify : {};
    const healthList = makeElement("ul", "health-list");
    for (const [label, observation] of [["Doctor", doctor], ["Verify", verify]]) {
      const row = makeElement("li", "health-row");
      row.append(makeElement("span", "", label), makeBadge(observation.status));
      if (observation.error_code) {
        row.title = safeErrorCode(observation.error_code);
      } else if (observation.observed_at) {
        row.title = displayValue(observation.observed_at);
      }
      healthList.append(row);
    }
    healthSection.append(healthList);

    const recallSection = makeSection("Last recall", "Most recent persisted bundle");
    const recall = isRecord(observability.last_recall) ? observability.last_recall : null;
    if (!recall) {
      recallSection.append(makeElement("p", "empty-inline", "No recall bundle recorded."));
    } else {
      const bundleButton = makeButton(
        displayValue(recall.bundle_id),
        (event) => openBundleDetails(recall.bundle_id, event.currentTarget),
        "text-button code-value",
      );
      recallSection.append(makeDefinitionList([
        { label: "Bundle", node: bundleButton },
        { label: "Timestamp", value: recall.timestamp },
        { label: "Outcome", node: makeBadge(recall.outcome) },
        { label: "Duration", value: displayDuration(recall.duration_ms) },
        { label: "Continuations", value: displayNumber(recall.continuation_count) },
        { label: "More relevant memory", value: recall.more_results === true ? "Yes" : "No" },
      ]));
    }
    columns.append(healthSection, recallSection);
    content.append(columns);

    const errorSection = makeSection("Last errors", "Failure, timeout, and overflow outcomes");
    errorSection.classList.add("full-width");
    const errors = asArray(observability.last_errors);
    if (errors.length === 0) {
      errorSection.append(makeElement("p", "empty-inline", "No recorded errors."));
    } else {
      errorSection.append(makeActivityTable(errors, "Latest local errors"));
      if (observability.last_errors_more_results === true) {
        errorSection.append(makeNotice(
          "More error events exist. Open Activity to inspect bounded pages.",
        ));
      }
    }
    content.append(errorSection);

    const inventoryGrid = makeElement("div", "section-grid");
    const typeSection = makeSection("Nodes by type", "Complete inventory counts");
    typeSection.append(makeNamedCountTable(
      asArray(memory.counts_by_type),
      "Node counts by type",
    ));
    const statusSection = makeSection("Nodes by status", "Current lifecycle states");
    statusSection.append(makeNamedCountTable(
      asArray(memory.counts_by_status),
      "Node counts by status",
    ));
    inventoryGrid.append(typeSection, statusSection);
    content.append(inventoryGrid);

    elements.viewContent.replaceChildren(content);
    setReadyState(false, false);
    announce(`Overview loaded. ${nodeCount} nodes and ${linkCount} links.`);
  }

  function makeNamedCountTable(items, caption) {
    if (items.length === 0) {
      return makeElement("p", "empty-inline", "No counts available.");
    }
    const table = makeTable(caption, ["Name", "Count"]);
    for (const item of items) {
      const row = makeElement("tr");
      appendCell(row, item.name, "cell-mono");
      appendCell(row, displayNumber(item.count));
      table.body.append(row);
    }
    return table.wrapper;
  }

  async function loadMemory(request) {
    const page = state.pages.memory;
    const filters = state.filters.memory;
    const response = await requestJson("memory", {
      limit: PAGE_LIMIT,
      cursor: page.cursor,
      type: filters.nodeType,
      status: filters.status,
      q: filters.search,
    }, request.signal);
    if (!requestIsCurrent(state.viewRequest, request)) {
      return;
    }
    const items = validateBoundedItems(response, "items", MAX_PAGE_SIZE);
    updatePage(page, response);
    page.itemCount = items.length;
    renderMemory(response, items);
  }

  function renderMemory(response, items) {
    const fragment = document.createDocumentFragment();
    fragment.append(makeMemoryToolbar());
    if (response.complete === false) {
      fragment.append(makeNotice(
        "This is a bounded page. Use Next to continue without silent truncation.",
      ));
    }
    if (items.length === 0) {
      fragment.append(makeElement(
        "p",
        "empty-inline",
        "No memory nodes match the current page and filters.",
      ));
    } else {
      const table = makeTable(
        "Memory nodes",
        ["ID", "Title", "Type", "Status", "Summary", "Trust", "Updated"],
      );
      for (const item of items) {
        const row = makeElement("tr");
        appendCell(row, item.id, "cell-mono");
        const titleCell = makeElement("td", "cell-title");
        titleCell.append(makeButton(
          displayValue(item.title, "Untitled"),
          (event) => openNodeDetails(item.id, event.currentTarget),
          "text-button",
        ));
        row.append(titleCell);
        appendCell(row, item.node_type, "cell-mono");
        const statusCell = makeElement("td");
        statusCell.append(makeBadge(item.status));
        row.append(statusCell);
        appendCell(row, item.summary, "cell-summary");
        appendCell(row, item.trust_level, "cell-mono");
        appendCell(row, item.updated_at, "cell-mono");
        table.body.append(row);
      }
      fragment.append(table.wrapper);
    }
    fragment.append(makePagination(
      "Page",
      state.pages.memory,
      () => showView("memory", false),
    ));
    elements.viewContent.replaceChildren(fragment);
    const partial = response.complete === false;
    setReadyState(items.length === 0, partial);
    announce(`Memory page loaded. ${items.length} nodes shown.`);
  }

  function makeMemoryToolbar() {
    const form = makeElement("form", "toolbar");
    form.setAttribute("aria-label", "Memory filters");
    const typeField = makeSelectField(
      "memory-type",
      "Type",
      NODE_TYPES,
      state.filters.memory.nodeType,
      "All types",
    );
    const statusField = makeSelectField(
      "memory-status",
      "Status",
      NODE_STATUSES,
      state.filters.memory.status,
      "All statuses",
    );
    const searchField = makeInputField(
      "memory-search",
      "Search",
      state.filters.memory.search,
      "Title, summary, or body terms",
      true,
    );
    searchField.input.maxLength = MAX_SEARCH_BYTES;
    form.append(
      typeField.wrapper,
      statusField.wrapper,
      searchField.wrapper,
      makeSubmitButton("Apply"),
    );
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      const search = searchField.input.value.trim();
      const byteLength = new TextEncoder().encode(search).length;
      if (byteLength > MAX_SEARCH_BYTES) {
        searchField.input.setCustomValidity(
          `Search must be at most ${MAX_SEARCH_BYTES} UTF-8 bytes.`,
        );
        searchField.input.reportValidity();
        return;
      }
      searchField.input.setCustomValidity("");
      state.filters.memory = {
        nodeType: typeField.select.value,
        status: statusField.select.value,
        search,
      };
      resetPage(state.pages.memory);
      showView("memory", false);
    });
    return form;
  }

  async function openNodeDetails(nodeId, trigger, preserveLinkPage = false) {
    const numericId = Number(nodeId);
    if (!Number.isSafeInteger(numericId) || numericId <= 0) {
      return;
    }
    if (!preserveLinkPage || state.nodeDetailId !== numericId) {
      resetPage(state.pages.nodeLinks);
    }
    state.nodeDetailId = numericId;
    openDetail(`Node ${numericId}`, trigger || state.detailReturnFocus);
    const request = beginRequest(state.detailRequest);
    const linkPage = state.pages.nodeLinks;
    try {
      const [nodeResponse, linksResponse] = await Promise.all([
        requestJson("node", { id: numericId }, request.signal),
        requestJson("node-links", {
          id: numericId,
          limit: PAGE_LIMIT,
          cursor: linkPage.cursor,
          direction: "both",
        }, request.signal),
      ]);
      if (!requestIsCurrent(state.detailRequest, request)) {
        return;
      }
      const links = validateBoundedItems(linksResponse, "items", MAX_PAGE_SIZE);
      updatePage(linkPage, linksResponse);
      linkPage.itemCount = links.length;
      renderNodeDetail(nodeResponse.node, linksResponse, links);
    } catch (error) {
      if (!isAbortError(error) && requestIsCurrent(state.detailRequest, request)) {
        renderError(
          error,
          () => openNodeDetails(numericId, state.detailReturnFocus, true),
          elements.detailContent,
        );
      }
    }
  }

  function renderNodeDetail(node, linksResponse, links) {
    if (!isRecord(node)) {
      throw new UiRequestError("UI_RESPONSE_INVALID", "Node details are invalid.");
    }
    elements.detailTitle.textContent = displayValue(node.title, `Node ${node.id}`);
    const fragment = document.createDocumentFragment();
    fragment.append(makeDefinitionList([
      { label: "ID", value: node.id },
      { label: "Type", value: node.node_type },
      { label: "Status", node: makeBadge(node.status) },
      { label: "Source", value: node.source_ref },
      { label: "Trust", value: node.trust_level },
      { label: "Confidence", value: displayDecimal(node.confidence) },
      { label: "Created", value: node.created_at },
      { label: "Updated", value: node.updated_at },
    ]));
    if (node.summary) {
      fragment.append(makeElement("p", "detail-summary", node.summary));
    }
    const bodySection = makeSection("Body", "Full content on explicit node read");
    bodySection.append(makeElement(
      "pre",
      "body-content",
      node.body === null || node.body === undefined ? "No body stored." : node.body,
    ));
    fragment.append(bodySection);

    if (state.activeView === "graph") {
      const centerButton = makeButton(
        "Center graph on this node",
        () => {
          state.filters.graph.center = String(node.id);
          resetPage(state.pages.graph);
          closeDetail(false);
          showView("graph", false);
        },
        "button button-primary",
      );
      fragment.append(centerButton);
    }

    const linksSection = makeSection("Links", "Incoming and outgoing, ordered by link ID");
    if (linksResponse.complete === false) {
      linksSection.append(makeNotice(
        "More links exist. Continue with the bounded page controls.",
      ));
    }
    if (links.length === 0) {
      linksSection.append(makeElement("p", "empty-inline", "No links on this page."));
    } else {
      const table = makeTable(
        "Node links",
        ["Direction", "Type", "Source", "Target"],
      );
      for (const link of links) {
        const row = makeElement("tr");
        appendCell(row, link.direction);
        appendCell(row, link.link_type, "cell-mono");
        const sourceCell = makeElement("td");
        sourceCell.append(makeButton(
          displayValue(link.source_node_id),
          (event) => openNodeDetails(link.source_node_id, event.currentTarget),
          "text-button cell-mono",
        ));
        row.append(sourceCell);
        const targetCell = makeElement("td");
        targetCell.append(makeButton(
          displayValue(link.target_node_id),
          (event) => openNodeDetails(link.target_node_id, event.currentTarget),
          "text-button cell-mono",
        ));
        row.append(targetCell);
        table.body.append(row);
      }
      linksSection.append(table.wrapper);
    }
    linksSection.append(makePagination(
      "Link page",
      state.pages.nodeLinks,
      () => openNodeDetails(node.id, state.detailReturnFocus, true),
    ));
    fragment.append(linksSection);
    elements.detailContent.replaceChildren(fragment);
    elements.detailContent.setAttribute("aria-busy", "false");
    elements.detailTitle.focus();
    announce(`Node ${node.id} details loaded.`);
  }

  async function loadGraph(request) {
    const page = state.pages.graph;
    const filters = state.filters.graph;
    const response = await requestJson("graph", {
      limit: PAGE_LIMIT,
      cursor: page.cursor,
      type: filters.nodeType,
      status: filters.status,
      center: filters.center,
    }, request.signal);
    if (!requestIsCurrent(state.viewRequest, request)) {
      return;
    }
    const graph = normalizeGraphResponse(response, page.cursor, filters.center);
    updatePage(page, response, "nodes_next_cursor", "nodes_more_results");
    page.itemCount = graph.nodes.length;
    renderGraph(response, graph);
  }

  function normalizeGraphResponse(response, cursor, requestedCenter) {
    const pageNodes = validateBoundedItems(response, "nodes", MAX_GRAPH_NODES);
    const edges = validateBoundedItems(response, "edges", MAX_GRAPH_EDGES);
    if (!Number.isInteger(response.node_limit) || response.node_limit <= 0
        || response.node_limit > MAX_GRAPH_NODES
        || !Number.isInteger(response.edge_limit) || response.edge_limit <= 0
        || response.edge_limit > MAX_GRAPH_EDGES) {
      throw new UiRequestError(
        "UI_RESPONSE_TOO_LARGE",
        "The graph response exceeds the local UI bounds.",
      );
    }
    const centerNode = isRecord(response.center_node) ? response.center_node : null;
    const centerId = requestedCenter ? Number(requestedCenter) : null;
    const responseCenter = response.center === null || response.center === undefined
      ? null
      : Number(response.center);
    if (responseCenter !== centerId) {
      throw new UiRequestError(
        "UI_GRAPH_CENTER_MISMATCH",
        "The graph response does not match the requested center.",
      );
    }
    if (cursor && centerId && !centerNode) {
      throw new UiRequestError(
        "UI_GRAPH_CENTER_MISSING",
        "A centered continuation page did not include its center context.",
      );
    }
    if (centerId && (!centerNode || Number(centerNode.id) !== centerId)) {
      throw new UiRequestError(
        "UI_GRAPH_CENTER_MISMATCH",
        "The graph center does not match the requested context.",
      );
    }

    const deduplicated = new Map();
    if (centerNode) {
      deduplicated.set(Number(centerNode.id), centerNode);
    }
    for (const item of pageNodes) {
      const id = Number(item.id);
      if (Number.isSafeInteger(id) && id > 0 && !deduplicated.has(id)) {
        deduplicated.set(id, item);
      }
    }
    if (deduplicated.size > MAX_GRAPH_NODES) {
      throw new UiRequestError(
        "UI_RESPONSE_TOO_LARGE",
        "The graph node set exceeds the local UI bounds.",
      );
    }
    const nodes = Array.from(deduplicated.values()).sort(compareNodes);
    const visibleEdges = edges
      .filter((edge) => deduplicated.has(Number(edge.source_node_id))
        && deduplicated.has(Number(edge.target_node_id)))
      .sort(compareEdges);
    return {
      nodes,
      edges: visibleEdges,
      omittedEdges: edges.length - visibleEdges.length,
      centerId: centerNode ? Number(centerNode.id) : null,
    };
  }

  function compareNodes(left, right) {
    return Number(left.id) - Number(right.id)
      || displayValue(left.node_type).localeCompare(displayValue(right.node_type))
      || displayValue(left.status).localeCompare(displayValue(right.status))
      || displayValue(left.title).localeCompare(displayValue(right.title));
  }

  function compareEdges(left, right) {
    return Number(left.source_node_id) - Number(right.source_node_id)
      || Number(left.target_node_id) - Number(right.target_node_id)
      || displayValue(left.link_type).localeCompare(displayValue(right.link_type))
      || Number(left.id) - Number(right.id);
  }

  function renderGraph(response, graph) {
    const fragment = document.createDocumentFragment();
    fragment.append(makeGraphToolbar());
    const meta = makeElement("div", "graph-meta");
    meta.append(
      makeElement("span", "", `${graph.nodes.length} nodes`),
      makeElement("span", "", `${graph.edges.length} visible edges`),
      makeElement("span", "", graph.centerId ? `Center: ${graph.centerId}` : "No center filter"),
      makeElement("span", "", `Node cap: ${MAX_GRAPH_NODES}`),
      makeElement("span", "", `Edge cap: ${MAX_GRAPH_EDGES}`),
    );
    fragment.append(meta);
    if (response.complete === false) {
      const reasons = [];
      if (response.nodes_complete === false) {
        reasons.push("more nodes are available through Next");
      }
      if (response.edges_complete === false) {
        reasons.push("the edge set reached its hard bound; refine filters or choose a center");
      }
      fragment.append(makeNotice(`Bounded graph: ${reasons.join("; ")}.`));
    }
    if (graph.omittedEdges > 0) {
      fragment.append(makeNotice(
        `${graph.omittedEdges} edges referenced nodes outside this page and were not drawn.`,
        "info",
      ));
    }
    if (graph.nodes.length === 0) {
      fragment.append(makeElement(
        "p",
        "empty-inline",
        "No graph nodes match the current page and filters.",
      ));
    } else {
      fragment.append(makeGraphSvg(graph.nodes, graph.edges, graph.centerId));
      fragment.append(makeAccessibleGraphList(graph.nodes, graph.centerId));
    }
    fragment.append(makePagination(
      "Node page",
      state.pages.graph,
      () => showView("graph", false),
      "More node pages are available.",
    ));
    elements.viewContent.replaceChildren(fragment);
    const partial = response.complete === false;
    setReadyState(graph.nodes.length === 0, partial);
    announce(`Graph loaded. ${graph.nodes.length} nodes and ${graph.edges.length} edges.`);
  }

  function makeGraphToolbar() {
    const form = makeElement("form", "toolbar");
    form.setAttribute("aria-label", "Graph filters");
    const typeField = makeSelectField(
      "graph-type",
      "Type",
      NODE_TYPES,
      state.filters.graph.nodeType,
      "All types",
    );
    const statusField = makeSelectField(
      "graph-status",
      "Status",
      NODE_STATUSES,
      state.filters.graph.status,
      "All statuses",
    );
    const centerField = makeInputField(
      "graph-center",
      "Center node ID",
      state.filters.graph.center,
      "Optional positive ID",
    );
    centerField.input.inputMode = "numeric";
    centerField.input.pattern = "[0-9]+";
    form.append(
      typeField.wrapper,
      statusField.wrapper,
      centerField.wrapper,
      makeSubmitButton("Apply"),
    );
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      const center = centerField.input.value.trim();
      if (center && (!/^[1-9][0-9]*$/.test(center) || !Number.isSafeInteger(Number(center)))) {
        centerField.input.setCustomValidity("Center must be a positive node ID.");
        centerField.input.reportValidity();
        return;
      }
      centerField.input.setCustomValidity("");
      state.filters.graph = {
        nodeType: typeField.select.value,
        status: statusField.select.value,
        center,
      };
      resetPage(state.pages.graph);
      showView("graph", false);
    });
    return form;
  }

  function graphLayout(nodes, edges, centerId) {
    const nodeMap = new Map(nodes.map((node) => [Number(node.id), node]));
    const adjacency = new Map(nodes.map((node) => [Number(node.id), new Set()]));
    for (const edge of edges) {
      const source = Number(edge.source_node_id);
      const target = Number(edge.target_node_id);
      if (adjacency.has(source) && adjacency.has(target)) {
        adjacency.get(source).add(target);
        adjacency.get(target).add(source);
      }
    }
    const sortedNodes = nodes;
    const rootId = centerId && nodeMap.has(centerId)
      ? centerId
      : Number(sortedNodes[0].id);
    const depth = new Map([[rootId, 0]]);
    const queue = [rootId];
    for (let index = 0; index < queue.length; index += 1) {
      const current = queue[index];
      const neighbors = Array.from(adjacency.get(current) || []).sort((a, b) => a - b);
      for (const neighbor of neighbors) {
        if (!depth.has(neighbor)) {
          depth.set(neighbor, depth.get(current) + 1);
          queue.push(neighbor);
        }
      }
    }
    const reachedDepths = Array.from(depth.values());
    const disconnectedDepth = (reachedDepths.length ? Math.max(...reachedDepths) : 0) + 1;
    for (const node of sortedNodes) {
      const id = Number(node.id);
      if (!depth.has(id)) {
        depth.set(id, disconnectedDepth);
      }
    }

    const layers = new Map();
    for (const node of sortedNodes) {
      const nodeDepth = depth.get(Number(node.id));
      if (!layers.has(nodeDepth)) {
        layers.set(nodeDepth, []);
      }
      layers.get(nodeDepth).push(node);
    }
    const orderedLayers = Array.from(layers.entries()).sort((left, right) => left[0] - right[0]);
    for (const layer of orderedLayers) {
      layer[1].sort((left, right) =>
        displayValue(left.node_type).localeCompare(displayValue(right.node_type))
        || displayValue(left.status).localeCompare(displayValue(right.status))
        || displayValue(left.title).localeCompare(displayValue(right.title))
        || Number(left.id) - Number(right.id));
    }
    const maximumLayerSize = Math.max(...orderedLayers.map((layer) => layer[1].length), 1);
    const width = Math.max(900, 192 + (orderedLayers.length - 1) * 230);
    const height = Math.max(520, 80 + maximumLayerSize * 74);
    const positions = new Map();
    orderedLayers.forEach((layer, layerIndex) => {
      const x = orderedLayers.length === 1
        ? width / 2
        : 96 + layerIndex * ((width - 192) / (orderedLayers.length - 1));
      layer[1].forEach((node, itemIndex) => {
        const y = 40 + (itemIndex + 1) * ((height - 80) / (layer[1].length + 1));
        positions.set(Number(node.id), { x, y });
      });
    });
    return { width, height, positions };
  }

  function makeGraphSvg(nodes, edges, centerId) {
    const layout = graphLayout(nodes, edges, centerId);
    const viewport = makeElement("div", "graph-viewport");
    const svg = makeSvgElement("svg", "graph-svg");
    svg.setAttribute("viewBox", `0 0 ${layout.width} ${layout.height}`);
    svg.setAttribute("width", String(layout.width));
    svg.setAttribute("height", String(layout.height));
    svg.setAttribute("role", "img");
    svg.setAttribute("aria-label", "Bounded AOPMem memory graph");
    const title = makeSvgElement("title");
    title.textContent = `Memory graph with ${nodes.length} nodes and ${edges.length} edges`;
    svg.append(title);

    const definitions = makeSvgElement("defs");
    const marker = makeSvgElement("marker");
    marker.id = "graph-arrow";
    marker.setAttribute("viewBox", "0 0 10 10");
    marker.setAttribute("refX", "9");
    marker.setAttribute("refY", "5");
    marker.setAttribute("markerWidth", "6");
    marker.setAttribute("markerHeight", "6");
    marker.setAttribute("orient", "auto-start-reverse");
    const arrow = makeSvgElement("path", "graph-arrow-head");
    arrow.setAttribute("d", "M 0 0 L 10 5 L 0 10 z");
    marker.append(arrow);
    definitions.append(marker);
    svg.append(definitions);

    for (const edge of edges) {
      const source = layout.positions.get(Number(edge.source_node_id));
      const target = layout.positions.get(Number(edge.target_node_id));
      if (!source || !target) {
        continue;
      }
      const path = makeSvgElement("path", "graph-edge");
      const horizontal = Math.abs(target.x - source.x);
      const direction = target.x >= source.x ? 1 : -1;
      const startX = source.x + direction * 84;
      const endX = target.x - direction * 84;
      const control = Math.max(horizontal * 0.45, 54);
      path.setAttribute(
        "d",
        `M ${startX} ${source.y} C ${startX + direction * control} ${source.y}, `
          + `${endX - direction * control} ${target.y}, ${endX} ${target.y}`,
      );
      const edgeTitle = makeSvgElement("title");
      edgeTitle.textContent = `${displayValue(edge.link_type)}: `
        + `${edge.source_node_id} to ${edge.target_node_id}`;
      path.append(edgeTitle);
      svg.append(path);
    }

    for (const node of nodes) {
      const id = Number(node.id);
      const position = layout.positions.get(id);
      if (!position) {
        continue;
      }
      const group = makeSvgElement(
        "g",
        id === centerId ? "graph-node graph-node-center" : "graph-node",
      );
      group.setAttribute("transform", `translate(${position.x - 84} ${position.y - 22})`);
      group.setAttribute("tabindex", "0");
      group.setAttribute("role", "button");
      group.setAttribute(
        "aria-label",
        `Node ${id}: ${displayValue(node.title)}. ${displayValue(node.node_type)}. `
          + `${displayValue(node.status)}.`,
      );
      const rectangle = makeSvgElement("rect");
      rectangle.setAttribute("width", "168");
      rectangle.setAttribute("height", "44");
      rectangle.setAttribute("rx", "6");
      const nodeTitle = makeSvgElement("title");
      nodeTitle.textContent = `${displayValue(node.title)} (${displayValue(node.node_type)})`;
      const titleText = makeSvgElement("text", "graph-node-title");
      titleText.setAttribute("x", "10");
      titleText.setAttribute("y", "18");
      titleText.textContent = truncateLabel(node.title);
      const typeText = makeSvgElement("text", "graph-node-type");
      typeText.setAttribute("x", "10");
      typeText.setAttribute("y", "34");
      typeText.textContent = `${displayValue(node.node_type)} · ${displayValue(node.status)}`;
      group.append(nodeTitle, rectangle, titleText, typeText);
      group.addEventListener("click", (event) => openNodeDetails(id, event.currentTarget));
      group.addEventListener("keydown", (event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          openNodeDetails(id, event.currentTarget);
        }
      });
      svg.append(group);
    }
    viewport.append(svg);
    return viewport;
  }

  function makeAccessibleGraphList(nodes, centerId) {
    const details = makeElement("details", "graph-accessible-list");
    details.append(makeElement("summary", "", "Accessible node list"));
    const list = makeElement("ul", "graph-node-list");
    for (const node of nodes) {
      const item = makeElement("li");
      const suffix = Number(node.id) === centerId ? " (center)" : "";
      item.append(makeButton(
        `${node.id} · ${displayValue(node.title)}${suffix}`,
        (event) => openNodeDetails(node.id, event.currentTarget),
        "text-button",
      ));
      list.append(item);
    }
    details.append(list);
    return details;
  }

  async function loadActivity(request) {
    const page = state.pages.activity;
    const filters = state.filters.activity;
    const response = await requestJson("activity", {
      limit: PAGE_LIMIT,
      cursor: page.cursor,
      event: filters.eventType,
      outcome: filters.outcome,
      command: filters.command,
    }, request.signal);
    if (!requestIsCurrent(state.viewRequest, request)) {
      return;
    }
    const items = validateBoundedItems(response, "items", MAX_PAGE_SIZE);
    updatePage(page, response);
    page.itemCount = items.length;
    renderActivity(response, items);
  }

  function renderActivity(response, items) {
    const fragment = document.createDocumentFragment();
    fragment.append(makeActivityToolbar());
    if (response.collection_status === "not_collected") {
      fragment.append(makeNotice(
        "Local Observability has not been collected for this workspace.",
        "info",
      ));
    }
    if (response.complete === false) {
      fragment.append(makeNotice(
        "This activity page is incomplete. Use Next to continue.",
      ));
    }
    if (items.length === 0) {
      fragment.append(makeElement(
        "p",
        "empty-inline",
        "No activity events match the current page and filters.",
      ));
    } else {
      fragment.append(makeActivityTable(items, "Local observability activity"));
    }
    fragment.append(makePagination(
      "Page",
      state.pages.activity,
      () => showView("activity", false),
    ));
    elements.viewContent.replaceChildren(fragment);
    const partial = response.complete === false;
    setReadyState(items.length === 0, partial);
    announce(`Activity loaded. ${items.length} events shown.`);
  }

  function makeActivityToolbar() {
    const form = makeElement("form", "toolbar");
    form.setAttribute("aria-label", "Activity filters");
    const eventField = makeSelectField(
      "activity-event",
      "Event",
      EVENT_TYPES,
      state.filters.activity.eventType,
      "All events",
    );
    const outcomeField = makeSelectField(
      "activity-outcome",
      "Outcome",
      EVENT_OUTCOMES,
      state.filters.activity.outcome,
      "All outcomes",
    );
    const commandField = makeInputField(
      "activity-command",
      "Command",
      state.filters.activity.command,
      "Exact command ID",
      true,
    );
    commandField.input.maxLength = 128;
    commandField.input.pattern = "[A-Za-z0-9._:-]+";
    form.append(
      eventField.wrapper,
      outcomeField.wrapper,
      commandField.wrapper,
      makeSubmitButton("Apply"),
    );
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      const command = commandField.input.value.trim();
      if (command && !/^[A-Za-z0-9._:-]{1,128}$/.test(command)) {
        commandField.input.setCustomValidity(
          "Command may use letters, digits, period, underscore, colon, and hyphen.",
        );
        commandField.input.reportValidity();
        return;
      }
      commandField.input.setCustomValidity("");
      state.filters.activity = {
        eventType: eventField.select.value,
        outcome: outcomeField.select.value,
        command,
      };
      resetPage(state.pages.activity);
      showView("activity", false);
    });
    return form;
  }

  function makeActivityTable(items, caption) {
    const table = makeTable(
      caption,
      ["Timestamp", "Event", "Command", "Outcome", "Duration / error", "Bundle"],
      "data-table timeline-table",
    );
    for (const item of items) {
      const row = makeElement("tr");
      appendCell(row, item.timestamp, "cell-mono");
      appendCell(row, item.event_type, "event-type");
      appendCell(row, item.command, "cell-mono");
      const outcomeCell = makeElement("td");
      outcomeCell.append(makeBadge(item.outcome));
      row.append(outcomeCell);
      appendCell(
        row,
        item.error_code || displayDuration(item.duration_ms),
        item.error_code ? "cell-mono" : "cell-muted",
      );
      const bundleCell = makeElement("td");
      if (item.bundle_id) {
        bundleCell.append(makeButton(
          item.bundle_id,
          (event) => openBundleDetails(item.bundle_id, event.currentTarget),
          "text-button cell-mono",
        ));
      } else {
        bundleCell.textContent = "—";
      }
      row.append(bundleCell);
      table.body.append(row);
    }
    return table.wrapper;
  }

  async function openBundleDetails(bundleId, trigger, preservePage = false) {
    if (typeof bundleId !== "string" || bundleId.length > 128) {
      return;
    }
    if (!preservePage || state.bundleId !== bundleId) {
      resetPage(state.pages.bundle);
    }
    state.bundleId = bundleId;
    openDetail("Recall bundle", trigger || state.detailReturnFocus);
    const request = beginRequest(state.detailRequest);
    const page = state.pages.bundle;
    try {
      const response = await requestJson("bundle", {
        id: bundleId,
        limit: PAGE_LIMIT,
        cursor: page.cursor,
      }, request.signal);
      if (!requestIsCurrent(state.detailRequest, request)) {
        return;
      }
      const nodes = validateBoundedItems(response, "nodes", MAX_PAGE_SIZE);
      updatePage(page, response);
      page.itemCount = nodes.length;
      renderBundleDetail(response, nodes);
    } catch (error) {
      if (!isAbortError(error) && requestIsCurrent(state.detailRequest, request)) {
        renderError(
          error,
          () => openBundleDetails(bundleId, state.detailReturnFocus, true),
          elements.detailContent,
        );
      }
    }
  }

  function renderBundleDetail(response, nodes) {
    const bundle = isRecord(response.bundle) ? response.bundle : {};
    elements.detailTitle.textContent = "Recall bundle";
    const fragment = document.createDocumentFragment();
    fragment.append(makeDefinitionList([
      { label: "Bundle ID", value: bundle.bundle_id },
      { label: "Timestamp", value: bundle.timestamp },
      { label: "Outcome", node: makeBadge(bundle.outcome) },
      { label: "Duration", value: displayDuration(bundle.duration_ms) },
      { label: "Continuations", value: displayNumber(bundle.continuation_count) },
      { label: "More relevant memory", value: bundle.more_results === true ? "Yes" : "No" },
      { label: "Error", value: bundle.error_code },
    ]));
    const section = makeSection("Selected nodes", "Safe summaries and selection reasons");
    if (response.complete === false) {
      section.append(makeNotice(
        "More selected nodes exist. Continue with the bounded page controls.",
      ));
    }
    if (nodes.length === 0) {
      section.append(makeElement("p", "empty-inline", "No selected nodes on this page."));
    } else {
      const table = makeTable(
        "Recall bundle nodes",
        ["Node", "Type", "Score", "Reasons"],
      );
      for (const node of nodes) {
        const row = makeElement("tr");
        const nodeCell = makeElement("td", "cell-title");
        nodeCell.append(makeButton(
          `${node.node_id} · ${displayValue(node.node_title)}`,
          (event) => openNodeDetails(node.node_id, event.currentTarget),
          "text-button",
        ));
        row.append(nodeCell);
        appendCell(row, node.node_type, "cell-mono");
        appendCell(row, displayDecimal(node.score, 3), "cell-mono");
        const reasonsCell = makeElement("td");
        const reasons = makeElement("div", "reason-list");
        for (const reason of asArray(node.selection_reasons)) {
          reasons.append(makeElement("span", "reason-chip", reason));
        }
        if (reasons.childElementCount === 0) {
          reasons.textContent = "—";
        }
        reasonsCell.append(reasons);
        row.append(reasonsCell);
        table.body.append(row);
      }
      section.append(table.wrapper);
    }
    section.append(makePagination(
      "Node page",
      state.pages.bundle,
      () => openBundleDetails(bundle.bundle_id, state.detailReturnFocus, true),
    ));
    fragment.append(section);
    elements.detailContent.replaceChildren(fragment);
    elements.detailContent.setAttribute("aria-busy", "false");
    elements.detailTitle.focus();
    announce(`Recall bundle loaded. ${nodes.length} selected nodes shown.`);
  }

  async function loadEffectiveness(request) {
    const response = await requestJson("effectiveness", {}, request.signal);
    if (!requestIsCurrent(state.viewRequest, request)) {
      return;
    }
    renderEffectiveness(response);
  }

  function renderEffectiveness(response) {
    const fragment = document.createDocumentFragment();
    const period = isRecord(response.period) ? response.period : {};
    const header = makeElement("div", "collection-status");
    header.append(
      makeBadge(response.collection_status),
      makeElement(
        "span",
        "",
        `${displayValue(period.start_at)} to ${displayValue(period.end_at)}`,
      ),
    );
    fragment.append(header);
    if (response.complete === false) {
      const notCollected = response.collection_status === "not_collected"
        || !isRecord(response.facts);
      const retentionTruncated = period.retention_truncated === true;
      const message = notCollected
        ? "The report is incomplete because Local Observability is not available."
        : retentionTruncated
          ? "The report is incomplete because retention removed part of the requested period."
          : "The report is incomplete according to the local collection metadata.";
      fragment.append(makeNotice(message, "info"));
    }
    if (!isRecord(response.facts)) {
      fragment.append(makeElement(
        "p",
        "empty-inline",
        "No effectiveness facts have been collected for this workspace.",
      ));
      elements.viewContent.replaceChildren(fragment);
      setReadyState(true, response.complete === false);
      announce("Effectiveness has no collected facts.");
      return;
    }

    const facts = response.facts;
    const recall = isRecord(facts.recall) ? facts.recall : {};
    fragment.append(makeMetricStrip([
      { label: "Recalls", value: displayNumber(recall.count), note: `${displayNumber(recall.failed)} failed` },
      { label: "Empty", value: displayNumber(recall.empty), note: "No relevant result" },
      { label: "Mandatory overflow", value: displayNumber(recall.mandatory_overflow), note: "Hard budget failures" },
      { label: "Continuation bundles", value: displayNumber(recall.continuation_bundles), note: `${displayNumber(recall.continuation_invocations)} invocations` },
      { label: "More-results bundles", value: displayNumber(recall.more_results_bundles), note: "Bundles with relevant continuation" },
      { label: "Terminal more-results", value: displayNumber(recall.terminal_more_results_bundles), note: "Budget ended before exhaustion" },
      { label: "FTS fallback", value: displayNumber(recall.fts_fallback_bundles), note: "Bundles using search" },
      { label: "Graph traversal", value: displayNumber(recall.graph_traversal_bundles), note: "Bundles using links" },
    ]));

    const selectionGrid = makeElement("div", "section-grid");
    const typeSection = makeSection("Selected nodes by type", "Bundle selections");
    typeSection.append(makeNamedCountTable(
      asArray(facts.nodes_selected_by_type),
      "Selected node counts by type",
    ));
    const feedbackSection = makeSection("Feedback", "Explicit bundle outcomes");
    const feedback = isRecord(facts.feedback) ? facts.feedback : {};
    feedbackSection.append(makeDefinitionList([
      { label: "Useful", value: displayNumber(feedback.useful) },
      { label: "Partial", value: displayNumber(feedback.partial) },
      { label: "Wrong", value: displayNumber(feedback.wrong) },
    ], "fact-grid"));
    selectionGrid.append(typeSection, feedbackSection);
    fragment.append(selectionGrid);

    const mostSelected = isRecord(facts.most_selected) ? facts.most_selected : {};
    const selectedSection = makeSection("Most selected memory", "Stable top lists");
    selectedSection.classList.add("full-width");
    const selectedGrid = makeElement("div", "section-grid");
    selectedGrid.append(
      makeTopSelectedSection("Workflows", mostSelected.workflows),
      makeTopSelectedSection("Tools", mostSelected.tools),
      makeTopSelectedSection("Failure modes", mostSelected.failure_modes),
    );
    selectedSection.append(selectedGrid);
    fragment.append(selectedSection);

    const tools = isRecord(facts.tools) ? facts.tools : {};
    const operationGrid = makeElement("div", "section-grid");
    const toolSection = makeSection("Tool runs", "Persisted outcomes");
    toolSection.append(makeDefinitionList([
      { label: "Success", value: displayNumber(tools.success) },
      { label: "Failure", value: displayNumber(tools.failure) },
      { label: "Timeout", value: displayNumber(tools.timeout) },
    ], "fact-grid"));
    toolSection.append(makeRepeatedToolErrors(tools.repeated_errors));

    const reflectionSection = makeSection("Reflection", "Proposal lifecycle facts");
    const reflection = isRecord(facts.reflection) ? facts.reflection : {};
    const reflectionItems = [];
    for (const label of ["proposed", "applied", "drafted"]) {
      const item = isRecord(reflection[label]) ? reflection[label] : {};
      reflectionItems.push({
        label,
        value: `${displayNumber(item.events)} events / ${displayNumber(item.items)} items`,
      });
    }
    reflectionSection.append(makeDefinitionList(reflectionItems));
    operationGrid.append(toolSection, reflectionSection);
    fragment.append(operationGrid);

    const correctionSection = makeSection(
      "Repeated corrections and failure modes",
      "Titles selected across bundles",
    );
    correctionSection.classList.add("full-width");
    correctionSection.append(makeRepeatedMemoryTable(
      facts.repeated_correction_failure_mode_titles,
    ));
    fragment.append(correctionSection);

    const healthSection = makeSection("Operational signals", "Counts, not a product score");
    const drift = isRecord(facts.adapter_drift_events) ? facts.adapter_drift_events : {};
    const failures = isRecord(facts.doctor_verify_failures)
      ? facts.doctor_verify_failures
      : {};
    const cleanup = isRecord(facts.artifact_cleanup_deletions)
      ? facts.artifact_cleanup_deletions
      : {};
    const mcp = isRecord(facts.mcp) ? facts.mcp : {};
    healthSection.append(makeDefinitionList([
      { label: "Adapter missing", value: displayNumber(drift.missing) },
      { label: "Adapter drifted", value: displayNumber(drift.drifted) },
      { label: "Adapter failed", value: displayNumber(drift.failed) },
      { label: "Pending audit", value: displayNumber(facts.pending_audit_events) },
      { label: "Doctor failures", value: displayNumber(failures.doctor) },
      { label: "Verify failures", value: displayNumber(failures.verify) },
      { label: "Cleanup events", value: displayNumber(cleanup.cleanup_events) },
      { label: "Deleted paths", value: displayNumber(cleanup.deleted_paths) },
      { label: "MCP missing", value: displayNumber(mcp.missing_status_observations) },
      { label: "MCP unverified", value: displayNumber(mcp.configured_unverified_status_observations) },
    ], "fact-grid"));
    fragment.append(healthSection);

    elements.viewContent.replaceChildren(fragment);
    setReadyState(false, response.complete === false);
    announce(`Effectiveness loaded. ${displayNumber(recall.count)} recalls reported.`);
  }

  function makeTopSelectedSection(title, topListValue) {
    const section = makeSection(title, "");
    const topList = isRecord(topListValue) ? topListValue : {};
    const items = asArray(topList.items);
    if (items.length === 0) {
      section.append(makeElement("p", "empty-inline", "No selections."));
      return section;
    }
    const table = makeTable(`${title} top selections`, ["Node", "Bundles"]);
    for (const item of items) {
      const row = makeElement("tr");
      appendCell(row, `${displayValue(item.node_id)} · ${displayValue(item.title)}`);
      appendCell(row, displayNumber(item.bundles));
      table.body.append(row);
    }
    section.append(table.wrapper);
    if (topList.more_results === true) {
      section.append(makeNotice(`More ${title.toLowerCase()} exist outside this top list.`));
    }
    return section;
  }

  function makeRepeatedToolErrors(value) {
    const topList = isRecord(value) ? value : {};
    const items = asArray(topList.items);
    if (items.length === 0) {
      return makeElement("p", "empty-inline", "No repeated tool errors.");
    }
    const table = makeTable(
      "Repeated tool errors",
      ["Tool", "Error", "Runs", "Last seen"],
    );
    for (const item of items) {
      const row = makeElement("tr");
      appendCell(row, item.tool_id, "cell-mono");
      appendCell(row, item.error_code, "cell-mono");
      appendCell(row, displayNumber(item.invocations));
      appendCell(row, item.last_seen_at, "cell-mono");
      table.body.append(row);
    }
    const wrapper = makeElement("div");
    wrapper.append(table.wrapper);
    if (topList.more_results === true) {
      wrapper.append(makeNotice("More repeated tool errors exist outside this top list."));
    }
    return wrapper;
  }

  function makeRepeatedMemoryTable(value) {
    const topList = isRecord(value) ? value : {};
    const items = asArray(topList.items);
    if (items.length === 0) {
      return makeElement("p", "empty-inline", "No repeated titles." );
    }
    const table = makeTable(
      "Repeated correction and failure mode titles",
      ["Type", "Title", "Selections", "Nodes", "Bundles"],
    );
    for (const item of items) {
      const row = makeElement("tr");
      appendCell(row, item.node_type, "cell-mono");
      appendCell(row, item.title);
      appendCell(row, displayNumber(item.selections));
      appendCell(row, displayNumber(item.distinct_nodes));
      appendCell(row, displayNumber(item.bundles));
      table.body.append(row);
    }
    const wrapper = makeElement("div");
    wrapper.append(table.wrapper);
    if (topList.more_results === true) {
      wrapper.append(makeNotice("More repeated titles exist outside this top list."));
    }
    return wrapper;
  }

  async function loadTools(request) {
    const toolsPage = state.pages.tools;
    const mcpPage = state.pages.mcp;
    const [toolsResult, mcpResult] = await Promise.allSettled([
      requestJson("tools", {
        limit: PAGE_LIMIT,
        cursor: toolsPage.cursor,
        status: state.filters.tools.status,
        side_effects: state.filters.tools.sideEffects,
      }, request.signal),
      requestJson("mcp", {
        limit: PAGE_LIMIT,
        cursor: mcpPage.cursor,
        status: state.filters.mcp.status,
        kind: state.filters.mcp.kind,
      }, request.signal),
    ]);
    if (!requestIsCurrent(state.viewRequest, request)) {
      return;
    }
    if (toolsResult.status === "rejected" && isAbortError(toolsResult.reason)) {
      throw toolsResult.reason;
    }
    if (mcpResult.status === "rejected" && isAbortError(mcpResult.reason)) {
      throw mcpResult.reason;
    }
    renderTools(toolsResult, mcpResult);
  }

  function renderTools(toolsResult, mcpResult) {
    const fragment = document.createDocumentFragment();
    const toolsSection = makeSection("Generated tools", "Read-only registered contracts");
    if (toolsResult.status === "fulfilled") {
      const response = toolsResult.value;
      const items = validateBoundedItems(response, "items", MAX_PAGE_SIZE);
      updatePage(state.pages.tools, response);
      state.pages.tools.itemCount = items.length;
      toolsSection.append(makeToolsToolbar());
      if (response.complete === false) {
        toolsSection.append(makeNotice("More tool contracts are available through Next."));
      }
      toolsSection.append(makeToolsTable(items));
      toolsSection.append(makePagination(
        "Tool page",
        state.pages.tools,
        () => showView("tools", false),
      ));
    } else {
      const errorContainer = makeElement("div");
      renderError(toolsResult.reason, () => showView("tools", false), errorContainer);
      toolsSection.append(errorContainer);
    }
    fragment.append(toolsSection);

    const mcpSection = makeSection("MCP profiles", "Local configuration status");
    mcpSection.classList.add("full-width");
    if (mcpResult.status === "fulfilled") {
      const response = mcpResult.value;
      const items = validateBoundedItems(response, "items", MAX_PAGE_SIZE);
      updatePage(state.pages.mcp, response);
      state.pages.mcp.itemCount = items.length;
      mcpSection.append(makeMcpToolbar());
      if (response.complete === false) {
        mcpSection.append(makeNotice("More MCP profiles are available through Next."));
      }
      mcpSection.append(makeMcpTable(items));
      mcpSection.append(makePagination(
        "MCP page",
        state.pages.mcp,
        () => showView("tools", false),
      ));
    } else {
      const errorContainer = makeElement("div");
      renderError(mcpResult.reason, () => showView("tools", false), errorContainer);
      mcpSection.append(errorContainer);
    }
    fragment.append(mcpSection);
    elements.viewContent.replaceChildren(fragment);
    const toolsFailed = toolsResult.status === "rejected";
    const mcpFailed = mcpResult.status === "rejected";
    const anyFailed = toolsFailed || mcpFailed;
    if (toolsFailed && mcpFailed) {
      setViewState("error", false);
    } else {
      setReadyState(false, anyFailed);
    }
    if (toolsFailed && mcpFailed) {
      announce("Tools and MCP data are unavailable.");
    } else if (toolsFailed) {
      announce("MCP data loaded. Tool data is unavailable.");
    } else if (mcpFailed) {
      announce("Tool data loaded. MCP data is unavailable.");
    } else {
      announce("Tools and MCP status loaded.");
    }
  }

  function makeToolsToolbar() {
    const form = makeElement("form", "toolbar");
    form.setAttribute("aria-label", "Tool filters");
    const statusField = makeSelectField(
      "tool-status",
      "Status",
      NODE_STATUSES,
      state.filters.tools.status,
      "All statuses",
    );
    const sideEffectsField = makeSelectField(
      "tool-side-effects",
      "Side effects",
      TOOL_SIDE_EFFECTS,
      state.filters.tools.sideEffects,
      "All side effects",
    );
    form.append(
      statusField.wrapper,
      sideEffectsField.wrapper,
      makeSubmitButton("Apply"),
    );
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      state.filters.tools = {
        status: statusField.select.value,
        sideEffects: sideEffectsField.select.value,
      };
      resetPage(state.pages.tools);
      showView("tools", false);
    });
    return form;
  }

  function makeToolsTable(items) {
    if (items.length === 0) {
      return makeElement("p", "empty-inline", "No tools match the current page and filters.");
    }
    const table = makeTable(
      "Generated tools",
      ["Tool", "Name", "Status", "Owner workflow", "Side effects", "Approval"],
    );
    for (const item of items) {
      const row = makeElement("tr");
      appendCell(row, item.tool_id, "cell-mono");
      appendCell(row, item.name);
      const statusCell = makeElement("td");
      statusCell.append(makeBadge(item.status));
      row.append(statusCell);
      appendCell(row, item.owner_workflow, "cell-mono");
      appendCell(row, item.side_effects, "cell-mono");
      appendCell(row, item.approval_requirement, "cell-mono");
      table.body.append(row);
    }
    return table.wrapper;
  }

  function makeMcpToolbar() {
    const form = makeElement("form", "toolbar");
    form.setAttribute("aria-label", "MCP filters");
    const statusField = makeSelectField(
      "mcp-status",
      "Status",
      MCP_STATUSES,
      state.filters.mcp.status,
      "All statuses",
    );
    const kindField = makeInputField(
      "mcp-kind",
      "Kind",
      state.filters.mcp.kind,
      "Exact kind",
      true,
    );
    kindField.input.maxLength = 256;
    kindField.input.pattern = "[A-Za-z0-9._:-]+";
    form.append(
      statusField.wrapper,
      kindField.wrapper,
      makeSubmitButton("Apply"),
    );
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      const kind = kindField.input.value.trim();
      if (kind && !/^[A-Za-z0-9._:-]{1,256}$/.test(kind)) {
        kindField.input.setCustomValidity(
          "Kind may use letters, digits, period, underscore, colon, and hyphen.",
        );
        kindField.input.reportValidity();
        return;
      }
      kindField.input.setCustomValidity("");
      state.filters.mcp = { status: statusField.select.value, kind };
      resetPage(state.pages.mcp);
      showView("tools", false);
    });
    return form;
  }

  function makeMcpTable(items) {
    if (items.length === 0) {
      return makeElement("p", "empty-inline", "No MCP profiles match the current page and filters.");
    }
    const table = makeTable(
      "MCP profiles",
      ["ID", "Name", "Kind", "Status", "Read", "Write", "Side effects", "Approval"],
    );
    for (const item of items) {
      const row = makeElement("tr");
      appendCell(row, item.id, "cell-mono");
      appendCell(row, item.name);
      appendCell(row, item.kind, "cell-mono");
      const statusCell = makeElement("td");
      statusCell.append(makeBadge(item.status));
      row.append(statusCell);
      appendCell(row, item.read_operations, "cell-summary");
      appendCell(row, item.write_operations, "cell-summary");
      appendCell(row, item.side_effects, "cell-mono");
      appendCell(row, item.approval_requirement, "cell-mono");
      table.body.append(row);
    }
    return table.wrapper;
  }

  const VIEW_LOADERS = Object.freeze({
    overview: loadOverview,
    memory: loadMemory,
    graph: loadGraph,
    activity: loadActivity,
    effectiveness: loadEffectiveness,
    tools: loadTools,
  });

  function setNavigationDisabled(disabled) {
    for (const item of elements.primaryNav.querySelectorAll("[data-view]")) {
      item.disabled = disabled;
    }
    elements.refreshView.disabled = disabled;
  }

  async function initialize() {
    setNavigationDisabled(true);
    renderLoading(elements.viewContent, "Connecting to the local AOPMem UI…");
    const request = beginRequest(state.bootRequest);
    try {
      const bootstrap = await requestJson("bootstrap", {}, request.signal);
      if (!requestIsCurrent(state.bootRequest, request)) {
        return;
      }
      const capabilities = asArray(bootstrap.capabilities);
      const required = [
        "overview", "memory", "node", "node_links", "graph", "activity",
        "bundle", "effectiveness", "tools", "mcp",
      ];
      if (bootstrap.read_only !== true
          || !required.every((capability) => capabilities.includes(capability))) {
        throw new UiRequestError(
          "UI_CAPABILITY_MISMATCH",
          "The local UI API does not provide the required read-only capabilities.",
        );
      }
      state.bootstrap = bootstrap;
      elements.workspaceName.textContent = displayValue(bootstrap.workspace_key);
      elements.productVersion.textContent = `v${displayValue(bootstrap.product_version)}`;
      document.documentElement.dataset.uiReady = "true";
      setNavigationDisabled(false);
      await showView(state.activeView, false);
    } catch (error) {
      if (!isAbortError(error) && requestIsCurrent(state.bootRequest, request)) {
        document.documentElement.dataset.uiReady = "error";
        elements.workspaceName.textContent = "Workspace unavailable";
        setNavigationDisabled(true);
        renderError(error, initialize);
      }
    }
  }

  for (const item of elements.primaryNav.querySelectorAll("[data-view]")) {
    item.addEventListener("click", () => showView(item.dataset.view, true));
  }
  elements.refreshView.addEventListener("click", () => showView(state.activeView, false));
  elements.closeDetail.addEventListener("click", () => closeDetail(true));
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !elements.detailPanel.hidden) {
      closeDetail(true);
    }
  });

  updateNavigation(state.activeView);
  initialize();
})();
