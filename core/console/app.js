const state = {
  emails: [],
  selectedEmailId: null,
  emailDetail: null,
  emailDetailView: "preview",
  emailPage: {
    total: 0,
    page: 1,
    pageSize: 20,
    query: "",
    archived: "active",
  },
  logs: [],
  workflows: [],
  selectedWorkflowId: null,
  workflowEditorMode: "existing",
  workflowDraft: null,
  workflowEditorBaseline: null,
  workflowDirty: false,
  runs: [],
  runSteps: [],
  generatedAccounts: [],
  selectedRunId: null,
  runPage: {
    total: 0,
    page: 1,
    pageSize: 20,
  },
  runFilters: {
    status: "",
    workflowId: "",
    workflowExact: false,
  },
  updateRate: 2000,
  refreshTimer: null,
  runsRefreshHandle: null,
  emailsRefreshHandle: null,
  detailToken: 0,
};

const emailTable = document.querySelector("#emailTable");
const emailCount = document.querySelector("#emailCount");
const emailFilterBadge = document.querySelector("#emailFilterBadge");
const emailSearchInput = document.querySelector("#emailSearch");
const emailArchiveFilterInput = document.querySelector("#emailArchiveFilter");
const emailPageSizeInput = document.querySelector("#emailPageSize");
const refreshEmailsButton = document.querySelector("#refreshEmails");
const clearEmailFiltersButton = document.querySelector("#clearEmailFilters");
const prevEmailPageButton = document.querySelector("#prevEmailPage");
const nextEmailPageButton = document.querySelector("#nextEmailPage");
const emailPageInfo = document.querySelector("#emailPageInfo");
const emailMeta = document.querySelector("#emailMeta");
const emailBodyText = document.querySelector("#emailBodyText");
const emailBodyHtml = document.querySelector("#emailBodyHtml");
const emailDetailBadge = document.querySelector("#emailDetailBadge");
const emailPreviewFrame = document.querySelector("#emailPreviewFrame");
const emailViewPreviewButton = document.querySelector("#emailViewPreview");
const emailViewTextButton = document.querySelector("#emailViewText");
const emailViewHtmlButton = document.querySelector("#emailViewHtml");
const copyEmailCodeButton = document.querySelector("#copyEmailCode");
const copyEmailLinkButton = document.querySelector("#copyEmailLink");
const copyEmailTextButton = document.querySelector("#copyEmailText");
const copyEmailHtmlButton = document.querySelector("#copyEmailHtml");
const healthText = document.querySelector("#healthText");
const tunnelBadge = document.querySelector("#tunnelBadge");
const tunnelHint = document.querySelector("#tunnelHint");
const logList = document.querySelector("#logList");
const workflowList = document.querySelector("#workflowList");
const workflowEditor = document.querySelector("#workflowEditor");
const workflowEditorBadge = document.querySelector("#workflowEditorBadge");
const workflowEditorHint = document.querySelector("#workflowEditorHint");
const workflowIdInput = document.querySelector("#workflowId");
const workflowKindInput = document.querySelector("#workflowKind");
const workflowTitleInput = document.querySelector("#workflowTitle");
const workflowSummaryInput = document.querySelector("#workflowSummary");
const workflowStatusInput = document.querySelector("#workflowStatus");
const workflowParamFields = document.querySelector("#workflowParamFields");
const workflowParamsPreviewInput = document.querySelector("#workflowParamsPreview");
const runList = document.querySelector("#runList");
const runFilterBadge = document.querySelector("#runFilterBadge");
const runStatusFilterInput = document.querySelector("#runStatusFilter");
const runWorkflowFilterInput = document.querySelector("#runWorkflowFilter");
const runWorkflowMatchInput = document.querySelector("#runWorkflowMatch");
const runPageSizeInput = document.querySelector("#runPageSize");
const runWorkflowOptions = document.querySelector("#runWorkflowOptions");
const clearRunFiltersButton = document.querySelector("#clearRunFilters");
const prevRunPageButton = document.querySelector("#prevRunPage");
const nextRunPageButton = document.querySelector("#nextRunPage");
const runPageInfo = document.querySelector("#runPageInfo");
const runMeta = document.querySelector("#runMeta");
const runSteps = document.querySelector("#runSteps");
const accountList = document.querySelector("#accountList");
const runDetailBadge = document.querySelector("#runDetailBadge");
const accountExport = document.querySelector("#accountExport");
const copyRunIdButton = document.querySelector("#copyRunId");
const copyRunStepsButton = document.querySelector("#copyRunSteps");
const copyRunAccountsButton = document.querySelector("#copyRunAccounts");
const exportRunSummaryButton = document.querySelector("#exportRunSummary");

const tunnelForm = document.querySelector("#tunnelForm");
const stopTunnelButton = document.querySelector("#stopTunnel");
const settingsForm = document.querySelector("#settingsForm");
const createWorkflowButton = document.querySelector("#createWorkflow");
const resetWorkflowButton = document.querySelector("#resetWorkflow");
const deleteWorkflowButton = document.querySelector("#deleteWorkflow");
const refreshRunsButton = document.querySelector("#refreshRuns");
const emptyCardTemplate = document.querySelector("#emptyCard");

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function formatTime(timestamp) {
  if (!timestamp) {
    return "-";
  }
  const normalized = timestamp > 1e12 ? timestamp : timestamp * 1000;
  return new Date(normalized).toLocaleString();
}

function levelClass(level) {
  return level === "success" ? "success" : level === "warn" ? "warn" : "info";
}

function statusClass(status) {
  return status === "success" || status === "ready" || status === "active" ? "ok" : status === "warn" ? "warn" : "";
}

function statusLabel(status) {
  switch (status) {
    case "ready":
      return "待执行";
    case "active":
      return "活跃";
    case "idle":
      return "空闲";
    case "running":
      return "运行中";
    case "success":
      return "成功";
    case "warn":
      return "警告";
    default:
      return status || "未知";
  }
}

function workflowHint(workflow) {
  const parameters = workflow.parameters || {};
  switch (workflow.kind) {
    case "account_generate":
      return `批量数量 ${parameters.batch_size || 10} / 域名 ${parameters.account_domain || "沿用系统设置"}`;
    case "data_cleanup":
      return `保留天数 ${parameters.days_to_keep || 7}`;
    case "status_report":
      return `统计窗口 ${parameters.report_window_hours || 24} 小时`;
    case "environment_check":
      return [
        parameters.require_env_secret_match ? "密钥对齐" : null,
        parameters.require_public_hub_url ? "公网地址" : null,
        parameters.require_webhook ? "回调地址" : null,
      ].filter(Boolean).join(" / ") || "基础校验";
    default:
      return "自定义参数";
  }
}

function cloneEmptyCard(message) {
  const node = emptyCardTemplate.content.firstElementChild.cloneNode(true);
  node.textContent = message;
  return node;
}

function cloneWorkflowDraft(workflow) {
  return {
    id: workflow.id,
    kind: workflow.kind,
    title: workflow.title,
    summary: workflow.summary,
    status: workflow.status,
    builtin: Boolean(workflow.builtin),
    parameters: structuredClone(workflow.parameters || {}),
  };
}

function serializeWorkflowDraft(draft) {
  return JSON.stringify({
    id: draft.id || "",
    kind: draft.kind || "account_generate",
    title: draft.title || "",
    summary: draft.summary || "",
    status: draft.status || "ready",
    parameters: sanitizeParameters(draft.parameters || {}),
  });
}

function commitWorkflowBaseline() {
  state.workflowEditorBaseline = serializeWorkflowDraft(state.workflowDraft || createDefaultWorkflowDraft());
  state.workflowDirty = false;
}

function refreshWorkflowDirtyState() {
  const current = serializeWorkflowDraft(state.workflowDraft || createDefaultWorkflowDraft());
  state.workflowDirty = current !== state.workflowEditorBaseline;
}

function confirmDiscardWorkflowChanges(actionLabel) {
  if (!state.workflowDirty) {
    return true;
  }

  return window.confirm(`当前工作流编辑器有未保存变更，确认继续${actionLabel}吗？未保存内容会丢失。`);
}

function createDefaultWorkflowDraft() {
  return {
    id: "",
    kind: "account_generate",
    title: "",
    summary: "",
    status: "ready",
    builtin: false,
    parameters: normalizeWorkflowParameters("account_generate", {}),
  };
}

function formatWorkflowParams(parameters) {
  return JSON.stringify(parameters || {}, null, 2);
}

function normalizeWorkflowParameters(kind, parameters = {}) {
  switch (kind) {
    case "account_generate":
      return {
        batch_size: Number(parameters.batch_size) > 0 ? Number(parameters.batch_size) : 10,
        account_domain: typeof parameters.account_domain === "string" ? parameters.account_domain : "",
      };
    case "data_cleanup":
      return {
        days_to_keep: Number(parameters.days_to_keep) > 0 ? Number(parameters.days_to_keep) : 7,
      };
    case "status_report":
      return {
        report_window_hours: Number(parameters.report_window_hours) > 0 ? Number(parameters.report_window_hours) : 24,
      };
    case "environment_check":
      return {
        require_env_secret_match: parameters.require_env_secret_match !== false,
        require_public_hub_url: parameters.require_public_hub_url !== false,
        require_webhook: Boolean(parameters.require_webhook),
      };
    default:
      return {};
  }
}

function buildParameterPayload(kind, values) {
  switch (kind) {
    case "account_generate":
      return {
        batch_size: Math.max(1, Number(values.batch_size) || 10),
        account_domain: values.account_domain.trim() || null,
      };
    case "data_cleanup":
      return {
        days_to_keep: Math.max(1, Number(values.days_to_keep) || 7),
      };
    case "status_report":
      return {
        report_window_hours: Math.max(1, Number(values.report_window_hours) || 24),
      };
    case "environment_check":
      return {
        require_env_secret_match: Boolean(values.require_env_secret_match),
        require_public_hub_url: Boolean(values.require_public_hub_url),
        require_webhook: Boolean(values.require_webhook),
      };
    default:
      return {};
  }
}

function sanitizeParameters(parameters) {
  return Object.fromEntries(
    Object.entries(parameters).filter(([, value]) => value !== null && value !== "")
  );
}

function renderWorkflowParamFields(parameters, kind) {
  switch (kind) {
    case "account_generate":
      workflowParamFields.innerHTML = `
        <div class="param-grid">
          <label>
            <span>批量数量</span>
            <input data-param="batch_size" type="number" min="1" max="500" value="${escapeHtml(parameters.batch_size || 10)}" />
          </label>
          <label>
            <span>账户域名</span>
            <input data-param="account_domain" type="text" placeholder="phantom.local" value="${escapeHtml(parameters.account_domain || "")}" />
          </label>
        </div>
      `;
      break;
    case "data_cleanup":
      workflowParamFields.innerHTML = `
        <div class="param-grid">
          <label>
            <span>保留天数</span>
            <input data-param="days_to_keep" type="number" min="1" max="365" value="${escapeHtml(parameters.days_to_keep || 7)}" />
          </label>
        </div>
      `;
      break;
    case "status_report":
      workflowParamFields.innerHTML = `
        <div class="param-grid">
          <label>
            <span>统计窗口小时数</span>
            <input data-param="report_window_hours" type="number" min="1" max="168" value="${escapeHtml(parameters.report_window_hours || 24)}" />
          </label>
        </div>
      `;
      break;
    case "environment_check":
      workflowParamFields.innerHTML = `
        <div class="param-fields">
          <label class="check-row">
            <div>
              <strong>校验 HUB_SECRET 对齐</strong>
              <p>要求环境变量与数据库中的授权密钥一致。</p>
            </div>
            <input data-param="require_env_secret_match" type="checkbox" ${parameters.require_env_secret_match ? "checked" : ""} />
          </label>
          <label class="check-row">
            <div>
              <strong>要求公网地址</strong>
              <p>执行时检查是否已登记 public_hub_url。</p>
            </div>
            <input data-param="require_public_hub_url" type="checkbox" ${parameters.require_public_hub_url ? "checked" : ""} />
          </label>
          <label class="check-row">
            <div>
              <strong>要求回调地址</strong>
              <p>执行时检查是否已配置可用回调地址。</p>
            </div>
            <input data-param="require_webhook" type="checkbox" ${parameters.require_webhook ? "checked" : ""} />
          </label>
        </div>
      `;
      break;
    default:
      workflowParamFields.innerHTML = "";
  }
}

function collectWorkflowParametersFromForm(kind) {
  const values = {};
  workflowParamFields.querySelectorAll("[data-param]").forEach((input) => {
    if (input.type === "checkbox") {
      values[input.dataset.param] = input.checked;
    } else {
      values[input.dataset.param] = input.value;
    }
  });

  return sanitizeParameters(buildParameterPayload(kind, values));
}

function syncWorkflowDraftFromForm() {
  if (!state.workflowDraft) {
    state.workflowDraft = createDefaultWorkflowDraft();
  }

  const kind = workflowKindInput.value;
  state.workflowDraft = {
    ...state.workflowDraft,
    id: workflowIdInput.value.trim(),
    kind,
    title: workflowTitleInput.value,
    summary: workflowSummaryInput.value,
    status: workflowStatusInput.value,
    parameters: collectWorkflowParametersFromForm(kind),
  };

  workflowParamsPreviewInput.value = formatWorkflowParams(state.workflowDraft.parameters);
  refreshWorkflowDirtyState();
  renderWorkflowEditorBadge();
}

function addLog(message, level = "info") {
  state.logs.unshift({
    time: new Date().toLocaleTimeString(),
    message: String(message),
    level: levelClass(level),
  });
  state.logs = state.logs.slice(0, 160);
  renderLogs();
}

function renderLogs() {
  logList.innerHTML = "";
  for (const entry of state.logs) {
    const row = document.createElement("div");
    row.className = `log-line ${entry.level}`;
    row.innerHTML = `<span class="log-time">[${escapeHtml(entry.time)}]</span><span class="log-msg">${escapeHtml(entry.message)}</span>`;
    logList.appendChild(row);
  }
}

function renderEmails() {
  emailTable.innerHTML = "";
  emailCount.textContent = String(state.emailPage.total);
  renderEmailFilterBadge();
  renderEmailPager();

  if (state.emails.length === 0) {
    const template = document.querySelector("#emptyEmailRow");
    emailTable.appendChild(template.content.cloneNode(true));
    return;
  }

  for (const email of state.emails) {
    const row = document.createElement("tr");
    row.className = email.id === state.selectedEmailId ? "is-selected" : "";
    const link = email.extracted_link || email.link || "";
    const archived = Boolean(email.is_archived);
    row.innerHTML = `
      <td>${escapeHtml(formatTime(email.created_at || Math.floor(Date.now() / 1000)))}</td>
      <td>${escapeHtml(email.from_addr || email.from || "-")}</td>
      <td>${escapeHtml(email.subject || "无主题")}</td>
      <td>${escapeHtml(email.extracted_code || email.code || "-")}</td>
      <td>${link ? `<a href="${escapeHtml(link)}" target="_blank" rel="noreferrer">${escapeHtml(link)}</a>` : "-"}</td>
      <td>
        <div class="table-actions">
          <button type="button" class="ghost tiny" data-email-action="view" data-email-id="${escapeHtml(email.id)}">详情</button>
          <button type="button" class="ghost tiny" data-email-action="archive" data-email-id="${escapeHtml(email.id)}" data-email-archived="${archived ? "true" : "false"}">${archived ? "取消归档" : "归档"}</button>
          <button type="button" class="ghost tiny danger" data-email-action="delete" data-email-id="${escapeHtml(email.id)}">删除</button>
        </div>
      </td>
    `;
    row.addEventListener("click", () => {
      void selectEmail(email.id);
    });

    row.querySelectorAll("[data-email-action]").forEach((button) => {
      button.addEventListener("click", async (event) => {
        event.stopPropagation();
        const action = button.dataset.emailAction;
        const emailId = button.dataset.emailId;
        if (!emailId) {
          return;
        }

        try {
          if (action === "view") {
            await selectEmail(emailId);
          } else if (action === "archive") {
            await toggleArchiveEmail(emailId, button.dataset.emailArchived === "true");
          } else if (action === "delete") {
            await deleteEmail(emailId);
          }
        } catch (error) {
          addLog(String(error), "warn");
        }
      });
    });

    emailTable.appendChild(row);
  }
}

function renderEmailDetail() {
  const email = state.emailDetail;

  if (!email) {
    emailDetailBadge.textContent = "未选择";
    emailDetailBadge.className = "badge idle";
    emailMeta.innerHTML = "";
    emailMeta.appendChild(cloneEmptyCard("选择一封邮件后显示详情"));
    emailBodyText.textContent = "暂无正文文本";
    emailBodyHtml.textContent = "暂无网页源码";
    emailPreviewFrame.srcdoc = "<!DOCTYPE html><html><body style='font-family:Segoe UI,PingFang SC,sans-serif;color:#5d6f91;padding:16px;'>暂无网页预览</body></html>";
    renderEmailDetailView();
    return;
  }

  emailDetailBadge.textContent = email.is_archived ? "已归档" : "未归档";
  emailDetailBadge.className = `badge ${email.is_archived ? "warn" : "ok"}`;

  const metaItems = [
    ["邮件编号", email.id],
    ["时间", formatTime(email.created_at)],
    ["发件人", email.from_addr],
    ["收件人", email.to_addr],
    ["主题", email.subject || "无主题"],
    ["验证码", email.extracted_code || "-"],
    ["链接", email.extracted_link || "-"],
    ["自定义文本", email.extracted_text || "-"],
  ];

  emailMeta.innerHTML = metaItems
    .map(([label, value]) => `
      <article class="meta-card">
        <span class="meta-key">${escapeHtml(label)}</span>
        <div class="meta-value">${escapeHtml(value || "-")}</div>
      </article>
    `)
    .join("");

  emailBodyText.textContent = email.body_text || "暂无正文文本";
  emailBodyHtml.textContent = email.body_html || "暂无网页源码";
  emailPreviewFrame.srcdoc = buildEmailPreviewDocument(email.body_html, email.body_text);
  renderEmailDetailView();
}

function buildEmailPreviewDocument(bodyHtml, bodyText) {
  if (bodyHtml && bodyHtml.trim()) {
    return bodyHtml;
  }

  const escapedText = escapeHtml(bodyText || "暂无正文文本").replaceAll("\n", "<br />");
  return `<!DOCTYPE html><html><body style="font-family:Segoe UI,PingFang SC,sans-serif;color:#10203d;padding:16px;line-height:1.6;">${escapedText}</body></html>`;
}

function renderEmailDetailView() {
  const view = state.emailDetailView;
  emailPreviewFrame.hidden = view !== "preview";
  emailBodyText.hidden = view !== "text";
  emailBodyHtml.hidden = view !== "html";

  emailViewPreviewButton.classList.toggle("is-active", view === "preview");
  emailViewTextButton.classList.toggle("is-active", view === "text");
  emailViewHtmlButton.classList.toggle("is-active", view === "html");
}

async function copyToClipboard(value, successLabel) {
  const text = String(value || "").trim();
  if (!text) {
    throw new Error(`${successLabel}为空，无法复制`);
  }

  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    addLog(`${successLabel}已复制`, "success");
    return;
  }

  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "readonly");
  textarea.style.position = "absolute";
  textarea.style.left = "-9999px";
  document.body.appendChild(textarea);
  textarea.select();
  const ok = document.execCommand("copy");
  document.body.removeChild(textarea);

  if (!ok) {
    throw new Error(`${successLabel}复制失败`);
  }

  addLog(`${successLabel}已复制`, "success");
}

function downloadTextFile(filename, content, mimeType = "text/plain;charset=utf-8") {
  const blob = new Blob([content], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  document.body.appendChild(anchor);
  anchor.click();
  document.body.removeChild(anchor);
  URL.revokeObjectURL(url);
}

function currentRun() {
  return state.runs.find((item) => item.id === state.selectedRunId) || null;
}

function serializeRunSteps() {
  if (state.runSteps.length === 0) {
    return "";
  }

  return state.runSteps
    .map((step) => `#${step.step_index} [${step.level}] ${formatTime(step.created_at)}\n${step.message}`)
    .join("\n\n");
}

function serializeRunAccounts() {
  if (state.generatedAccounts.length === 0) {
    return "";
  }

  return state.generatedAccounts
    .map((account) => `${account.address}\t${account.password}\t${account.status}\t${formatTime(account.created_at)}`)
    .join("\n");
}

function buildRunSummary(run) {
  return {
    run,
    steps: state.runSteps,
    accounts: state.generatedAccounts,
    exported_at: new Date().toISOString(),
  };
}

function emailArchivedParam() {
  switch (state.emailPage.archived) {
    case "archived":
      return "true";
    case "active":
      return "false";
    default:
      return null;
  }
}

function renderEmailFilterBadge() {
  const filters = [];
  if (state.emailPage.query.trim()) {
    filters.push(`搜索：${state.emailPage.query.trim()}`);
  }
  if (state.emailPage.archived === "archived") {
    filters.push("已归档");
  } else if (state.emailPage.archived === "all") {
    filters.push("全部");
  } else {
    filters.push("未归档");
  }

  const hasCustomFilter = Boolean(state.emailPage.query.trim()) || state.emailPage.archived !== "active";
  emailFilterBadge.textContent = `${filters.join(" / ")} / ${state.emailPage.total} 封`;
  emailFilterBadge.className = hasCustomFilter ? "badge ok" : "badge idle";
}

function renderEmailPager() {
  const page = state.emailPage.page;
  const pageSize = state.emailPage.pageSize;
  const total = state.emailPage.total;
  const totalPages = Math.max(1, Math.ceil(total / pageSize));
  emailPageInfo.textContent = `第 ${page} / ${totalPages} 页，共 ${total} 封`;
  prevEmailPageButton.disabled = page <= 1;
  nextEmailPageButton.disabled = page >= totalPages;
}

function queueEmailsRefresh(delay = 250) {
  clearTimeout(state.emailsRefreshHandle);
  state.emailsRefreshHandle = setTimeout(() => {
    void loadEmails(true);
  }, delay);
}

function renderWorkflows() {
  workflowList.innerHTML = "";

  if (state.workflows.length === 0) {
    workflowList.appendChild(cloneEmptyCard("暂无工作流定义"));
    return;
  }

  for (const workflow of state.workflows) {
    const card = document.createElement("article");
    card.className = `workflow-card ${workflow.id === state.selectedWorkflowId && state.workflowEditorMode === "existing" ? "active" : ""}`.trim();
    card.innerHTML = `
      <div class="workflow-head">
        <h3>${escapeHtml(workflow.title)}</h3>
        <span class="tag ${statusClass(workflow.status)}">${escapeHtml(statusLabel(workflow.status))}</span>
      </div>
      <p>${escapeHtml(workflow.summary)}</p>
      <div class="workflow-meta">
        <span class="tag">${escapeHtml(workflow.kind)}</span>
        <span class="tag">${workflow.builtin ? "内建" : "自定义"}</span>
      </div>
      <div class="workflow-foot">
        <span class="hint">${escapeHtml(workflowHint(workflow))}</span>
        <button type="button" data-trigger="${escapeHtml(workflow.id)}">执行</button>
      </div>
    `;

    card.addEventListener("click", () => {
      selectWorkflow(workflow.id);
    });

    const triggerButton = card.querySelector("[data-trigger]");
    triggerButton.addEventListener("click", async (event) => {
      event.stopPropagation();
      try {
        await triggerWorkflow(workflow.id);
      } catch (error) {
        addLog(String(error), "warn");
      }
    });

    workflowList.appendChild(card);
  }
}

function renderWorkflowEditor() {
  const draft = state.workflowDraft || createDefaultWorkflowDraft();
  const isBuiltin = Boolean(draft.builtin);
  const isNew = state.workflowEditorMode === "new";
  const normalizedParameters = normalizeWorkflowParameters(draft.kind, draft.parameters);

  workflowIdInput.value = draft.id || "";
  workflowKindInput.value = draft.kind || "account_generate";
  workflowTitleInput.value = draft.title || "";
  workflowSummaryInput.value = draft.summary || "";
  workflowStatusInput.value = draft.status || "ready";
  renderWorkflowParamFields(normalizedParameters, draft.kind);
  workflowParamsPreviewInput.value = formatWorkflowParams(normalizedParameters);
  state.workflowDraft = {
    ...draft,
    parameters: normalizedParameters,
  };

  workflowIdInput.readOnly = !isNew;
  workflowKindInput.disabled = isBuiltin;
  deleteWorkflowButton.hidden = isNew || isBuiltin;
  renderWorkflowEditorBadge();
}

function renderWorkflowEditorBadge() {
  const draft = state.workflowDraft || createDefaultWorkflowDraft();
  const isBuiltin = Boolean(draft.builtin);
  const isNew = state.workflowEditorMode === "new";

  workflowEditorBadge.textContent = state.workflowDirty
    ? "未保存变更"
    : isNew
      ? "新建草稿"
      : isBuiltin
        ? "内置定义"
        : "自定义定义";
  workflowEditorBadge.className = `badge ${state.workflowDirty ? "warn" : isNew ? "idle" : isBuiltin ? "ok" : ""}`.trim();
  workflowEditorHint.textContent = state.workflowDirty
    ? "当前草稿尚未保存。切换、新建或重置前会提示确认。"
    : isNew
      ? "新建时请填写唯一的工作流编号，参数预览需要保持合法。"
      : isBuiltin
        ? "内置工作流不允许修改类型，也不能删除。"
        : "自定义工作流可以修改标题、摘要、状态和参数。";
}

function selectWorkflow(workflowId) {
  if (!confirmDiscardWorkflowChanges("切换工作流")) {
    return;
  }

  const workflow = state.workflows.find((item) => item.id === workflowId);
  if (!workflow) {
    return;
  }

  state.selectedWorkflowId = workflow.id;
  state.workflowEditorMode = "existing";
  state.workflowDraft = cloneWorkflowDraft(workflow);
  commitWorkflowBaseline();
  renderWorkflows();
  renderWorkflowEditor();
}

function startCreateWorkflow() {
  if (!confirmDiscardWorkflowChanges("新建工作流")) {
    return;
  }

  state.selectedWorkflowId = null;
  state.workflowEditorMode = "new";
  state.workflowDraft = createDefaultWorkflowDraft();
  commitWorkflowBaseline();
  renderWorkflows();
  renderWorkflowEditor();
}

function syncWorkflowSelection(preferredSelectionId = null) {
  if (preferredSelectionId) {
    const matched = state.workflows.find((item) => item.id === preferredSelectionId);
    if (matched) {
      state.selectedWorkflowId = matched.id;
      state.workflowEditorMode = "existing";
      state.workflowDraft = cloneWorkflowDraft(matched);
      commitWorkflowBaseline();
      return;
    }
  }

  if (state.workflowEditorMode === "new" && state.workflowDraft) {
    return;
  }

  const selected = state.workflows.find((item) => item.id === state.selectedWorkflowId);
  if (selected) {
    state.workflowDraft = cloneWorkflowDraft(selected);
    commitWorkflowBaseline();
    return;
  }

  const first = state.workflows[0];
  if (first) {
    state.selectedWorkflowId = first.id;
    state.workflowEditorMode = "existing";
    state.workflowDraft = cloneWorkflowDraft(first);
    commitWorkflowBaseline();
  } else {
    state.selectedWorkflowId = null;
    state.workflowEditorMode = "new";
    state.workflowDraft = createDefaultWorkflowDraft();
    commitWorkflowBaseline();
  }
}

function collectWorkflowPayload() {
  syncWorkflowDraftFromForm();
  const parameters = state.workflowDraft?.parameters || {};

  const id = workflowIdInput.value.trim();
  const title = workflowTitleInput.value.trim();
  const summary = workflowSummaryInput.value.trim();

  if (!id) {
    throw new Error("工作流编号不能为空");
  }
  if (!title) {
    throw new Error("工作流标题不能为空");
  }
  if (!summary) {
    throw new Error("工作流摘要不能为空");
  }

  return {
    id,
    kind: workflowKindInput.value,
    title,
    summary,
    status: workflowStatusInput.value,
    parameters_json: JSON.stringify(parameters),
  };
}

function renderRuns() {
  runList.innerHTML = "";
  renderRunFilterBadge(state.runPage.total);
  renderRunWorkflowOptions();
  renderRunPager();

  if (state.runs.length === 0) {
    runList.appendChild(cloneEmptyCard("暂无运行记录"));
    renderRunDetails();
    return;
  }

  for (const run of state.runs) {
    const card = document.createElement("article");
    card.className = `run-card ${run.id === state.selectedRunId ? "active" : ""}`.trim();
    card.innerHTML = `
      <div class="run-head">
        <h3>${escapeHtml(run.workflow_title)}</h3>
        <span class="tag ${statusClass(run.status)}">${escapeHtml(statusLabel(run.status))}</span>
      </div>
      <p>${escapeHtml(run.message)}</p>
      <div class="run-meta-line">
        <span class="tag">${escapeHtml(formatTime(run.started_at))}</span>
        <span class="tag">${escapeHtml(run.workflow_id)}</span>
      </div>
    `;
    card.addEventListener("click", () => {
      void selectRun(run.id);
    });
    runList.appendChild(card);
  }
}

function renderRunFilterBadge(totalCount) {
  const hasStatusFilter = Boolean(state.runFilters.status);
  const hasWorkflowFilter = Boolean(state.runFilters.workflowId.trim());
  const hasFilters = hasStatusFilter || hasWorkflowFilter;

  if (!hasFilters) {
    runFilterBadge.textContent = "未筛选";
    runFilterBadge.className = "badge idle";
    return;
  }

  const filters = [];
  if (hasStatusFilter) {
    filters.push(`状态:${state.runFilters.status}`);
  }
  if (hasWorkflowFilter) {
    filters.push(`工作流:${state.runFilters.workflowId.trim()}(${state.runFilters.workflowExact ? "精确" : "模糊"})`);
  }

  runFilterBadge.textContent = `${filters.join(" / ")} / ${totalCount} 条`;
  runFilterBadge.className = "badge ok";
}

function renderRunWorkflowOptions() {
  const workflowIds = Array.from(new Set(state.runs.map((run) => run.workflow_id).filter(Boolean))).sort((left, right) => left.localeCompare(right));
  runWorkflowOptions.innerHTML = workflowIds
    .map((workflowId) => `<option value="${escapeHtml(workflowId)}"></option>`)
    .join("");
}

function renderRunPager() {
  const page = state.runPage.page;
  const pageSize = state.runPage.pageSize;
  const total = state.runPage.total;
  const totalPages = Math.max(1, Math.ceil(total / pageSize));
  runPageInfo.textContent = `第 ${page} / ${totalPages} 页，共 ${total} 条`;
  prevRunPageButton.disabled = page <= 1;
  nextRunPageButton.disabled = page >= totalPages;
}

function applyRunFilters() {
  state.runFilters.status = runStatusFilterInput.value;
  state.runFilters.workflowId = runWorkflowFilterInput.value;
  state.runFilters.workflowExact = runWorkflowMatchInput.value === "exact";
  state.runPage.page = 1;
  void loadRuns(false).catch((error) => {
    addLog(String(error), "warn");
  });
}

function renderRunDetails() {
  const run = state.runs.find((item) => item.id === state.selectedRunId);

  if (!run) {
    runDetailBadge.textContent = "等待选择记录";
    runDetailBadge.className = "badge idle";
    runMeta.innerHTML = "";
    runMeta.appendChild(cloneEmptyCard("选择一条运行记录后显示详情"));
    runSteps.innerHTML = "";
    runSteps.appendChild(cloneEmptyCard("暂无步骤"));
    accountList.innerHTML = "";
    accountList.appendChild(cloneEmptyCard("暂无生成产物"));
    accountExport.hidden = true;
    return;
  }

  runDetailBadge.textContent = statusLabel(run.status);
  runDetailBadge.className = `badge ${statusClass(run.status)}`.trim();

  const metaItems = [
    ["运行编号", run.id],
    ["工作流编号", run.workflow_id],
    ["开始时间", formatTime(run.started_at)],
    ["结束时间", formatTime(run.finished_at)],
    ["最终摘要", run.message],
  ];

  runMeta.innerHTML = metaItems
    .map(([label, value]) => `
      <article class="meta-card">
        <span class="meta-key">${escapeHtml(label)}</span>
        <div class="meta-value">${escapeHtml(value || "-")}</div>
      </article>
    `)
    .join("");

  runSteps.innerHTML = "";
  if (state.runSteps.length === 0) {
    runSteps.appendChild(cloneEmptyCard("当前运行还没有步骤记录"));
  } else {
    for (const step of state.runSteps) {
      const item = document.createElement("article");
      item.className = "step-item";
      item.innerHTML = `
        <div class="step-top">
          <span class="tag ${statusClass(step.level)}">${escapeHtml(step.level)}</span>
          <code>${escapeHtml(`#${step.step_index} @ ${formatTime(step.created_at)}`)}</code>
        </div>
        <div class="step-message">${escapeHtml(step.message)}</div>
      `;
      runSteps.appendChild(item);
    }
  }

  accountList.innerHTML = "";
  if (state.generatedAccounts.length === 0) {
    accountList.appendChild(cloneEmptyCard("当前运行没有生成产物"));
    accountExport.hidden = true;
  } else {
    for (const account of state.generatedAccounts) {
      const item = document.createElement("article");
      item.className = "account-item";
      item.innerHTML = `
        <div class="account-top">
          <code>${escapeHtml(account.address)}</code>
          <span class="tag ${statusClass(account.status)}">${escapeHtml(statusLabel(account.status))}</span>
        </div>
        <div class="account-extra">密码: <code>${escapeHtml(account.password)}</code></div>
        <div class="account-extra">生成时间: ${escapeHtml(formatTime(account.created_at))}</div>
      `;
      accountList.appendChild(item);
    }
    accountExport.href = `/api/workflow-runs/${encodeURIComponent(run.id)}/accounts/export`;
    accountExport.hidden = false;
  }
}

function fillSettings(settings) {
  document.querySelector("#webhookUrl").value = settings.webhook_url || "";
  document.querySelector("#updateRate").value = settings.update_rate || 2000;
  document.querySelector("#authSecret").value = settings.auth_secret || "";
  document.querySelector("#decodeDepth").value = settings.decode_depth || "深度扫描 / FULL_DEEP_SCAN";
  document.querySelector("#publicUrl").value = settings.public_hub_url || "";
  document.querySelector("#accountDomain").value = settings.account_domain || "";
}

function setUpdateRate(updateRate) {
  const normalized = Math.max(1000, Number(updateRate) || 2000);
  state.updateRate = normalized;

  if (state.refreshTimer) {
    clearInterval(state.refreshTimer);
  }

  state.refreshTimer = setInterval(() => {
    void refreshRuntime();
  }, normalized);
}

function updateTunnel(status) {
  document.querySelector("#tunnelPort").value = status.port || 4000;
  document.querySelector("#subdomain").value = status.subdomain || "";
  tunnelBadge.textContent = status.active ? "已登记" : "未登记";
  tunnelBadge.className = status.active ? "badge ok" : "badge idle";
  tunnelHint.textContent = status.active && status.url
    ? `当前登记地址：${status.url}（${status.provider || "手动"}）`
    : "把任意可访问当前中枢的公网地址登记到这里，邮件转发工作节点使用同一地址即可。";
}

function queueRunsRefresh(delay = 400) {
  clearTimeout(state.runsRefreshHandle);
  state.runsRefreshHandle = setTimeout(() => {
    void loadRuns(true);
  }, delay);
}

async function loadHealth() {
  try {
    const response = await fetch("/health");
    healthText.textContent = await response.text();
  } catch (error) {
    healthText.textContent = "不可用";
    addLog(`健康检查失败: ${error}`, "warn");
  }
}

async function loadEmails() {
  const search = new URLSearchParams();
  search.set("page", String(state.emailPage.page));
  search.set("page_size", String(state.emailPage.pageSize));
  if (state.emailPage.query.trim()) {
    search.set("q", state.emailPage.query.trim());
  }
  const archived = emailArchivedParam();
  if (archived !== null) {
    search.set("archived", archived);
  }

  const response = await fetch(`/api/emails/query?${search.toString()}`);
  if (!response.ok) {
    throw new Error("邮件列表读取失败");
  }
  const result = await response.json();
  state.emails = result.items || [];
  state.emailPage.total = result.total || 0;
  state.emailPage.page = result.page || state.emailPage.page;
  state.emailPage.pageSize = result.page_size || state.emailPage.pageSize;
  if (!state.emails.some((email) => email.id === state.selectedEmailId)) {
    state.selectedEmailId = null;
    state.emailDetail = null;
  }
  renderEmails();
  renderEmailDetail();
}

async function loadEmailDetail(emailId) {
  const response = await fetch(`/api/emails/${encodeURIComponent(emailId)}`);
  if (!response.ok) {
    throw new Error("邮件详情读取失败");
  }
  state.emailDetail = await response.json();
  renderEmailDetail();
}

async function selectEmail(emailId) {
  state.selectedEmailId = emailId;
  renderEmails();
  await loadEmailDetail(emailId);
}

async function toggleArchiveEmail(emailId, archived) {
  const response = await fetch(`/api/emails/${encodeURIComponent(emailId)}/archive`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ archived: !archived }),
  });
  const result = await response.json();
  if (!response.ok) {
    throw new Error(result.message || "邮件归档更新失败");
  }

  addLog(`${archived ? "邮件已取消归档" : "邮件已归档"}：${emailId}`, "success");
  if (state.selectedEmailId === emailId && state.emailDetail) {
    state.emailDetail.is_archived = !archived;
  }
  await loadEmails(true);
  if (state.selectedEmailId === emailId) {
    await loadEmailDetail(emailId).catch(() => {
      state.emailDetail = null;
      renderEmailDetail();
    });
  }
}

async function deleteEmail(emailId) {
  const confirmed = window.confirm(`确认删除邮件「${emailId}」吗？`);
  if (!confirmed) {
    return;
  }

  const response = await fetch(`/api/emails/${encodeURIComponent(emailId)}`, {
    method: "DELETE",
  });
  const result = await response.json();
  if (!response.ok) {
    throw new Error(result.message || "邮件删除失败");
  }

  addLog(`邮件已删除：${emailId}`, "success");
  if (state.selectedEmailId === emailId) {
    state.selectedEmailId = null;
    state.emailDetail = null;
  }
  await loadEmails(true);
}

async function loadSettings() {
  const response = await fetch("/api/settings");
  if (!response.ok) {
    throw new Error("系统设置读取失败");
  }
  const settings = await response.json();
  fillSettings(settings);
  setUpdateRate(settings.update_rate);
}

async function loadTunnel() {
  const response = await fetch("/api/tunnel/status");
  if (!response.ok) {
    throw new Error("公网登记状态读取失败");
  }
  const status = await response.json();
  updateTunnel(status);
}

async function loadWorkflows(preferredSelectionId = null) {
  const response = await fetch("/api/workflows");
  if (!response.ok) {
    throw new Error("工作流定义读取失败");
  }
  state.workflows = await response.json();
  syncWorkflowSelection(preferredSelectionId);
  renderWorkflows();
  renderWorkflowEditor();
}

async function loadRunDetails(runId) {
  if (!runId) {
    state.runSteps = [];
    state.generatedAccounts = [];
    renderRunDetails();
    return;
  }

  const token = ++state.detailToken;
  const [stepsResponse, accountsResponse] = await Promise.all([
    fetch(`/api/workflow-runs/${encodeURIComponent(runId)}/steps`),
    fetch(`/api/workflow-runs/${encodeURIComponent(runId)}/accounts`),
  ]);

  if (token !== state.detailToken) {
    return;
  }

  if (!stepsResponse.ok || !accountsResponse.ok) {
    throw new Error("运行详情读取失败");
  }

  state.runSteps = await stepsResponse.json();
  state.generatedAccounts = await accountsResponse.json();
  renderRunDetails();
}

async function selectRun(runId) {
  state.selectedRunId = runId;
  renderRuns();
  renderRunDetails();
  try {
    await loadRunDetails(runId);
  } catch (error) {
    addLog(String(error), "warn");
  }
}

async function loadRuns(preserveSelection = true) {
  const search = new URLSearchParams();
  search.set("page", String(state.runPage.page));
  search.set("page_size", String(state.runPage.pageSize));
  if (state.runFilters.status) {
    search.set("status", state.runFilters.status);
  }
  if (state.runFilters.workflowId.trim()) {
    search.set("workflow_id", state.runFilters.workflowId.trim());
    search.set("workflow_exact", state.runFilters.workflowExact ? "true" : "false");
  }

  const response = await fetch(`/api/workflow-runs?${search.toString()}`);
  if (!response.ok) {
    throw new Error("运行记录读取失败");
  }

  const result = await response.json();
  state.runs = result.items || [];
  state.runPage.total = result.total || 0;
  state.runPage.page = result.page || state.runPage.page;
  state.runPage.pageSize = result.page_size || state.runPage.pageSize;

  if (!preserveSelection || !state.runs.some((item) => item.id === state.selectedRunId)) {
    state.selectedRunId = state.runs[0]?.id || null;
  }

  renderRuns();
  renderRunDetails();

  if (state.selectedRunId) {
    await loadRunDetails(state.selectedRunId);
  }
}

async function refreshRuntime() {
  await Promise.allSettled([
    loadHealth(),
    loadTunnel(),
    loadEmails(true),
    loadRuns(true),
  ]);
}

async function triggerWorkflow(workflowId) {
  const response = await fetch("/api/workflows/trigger", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ workflow_id: workflowId }),
  });

  const result = await response.json();
  if (!response.ok) {
    throw new Error(result.message || "工作流触发失败");
  }

  addLog(`工作流已触发：${workflowId}，运行编号：${result.run_id}`, "success");
  await loadRuns(false);

  if (result.run_id) {
    await selectRun(result.run_id);
  }
}

tunnelForm.addEventListener("submit", async (event) => {
  event.preventDefault();

  const payload = {
    port: Number(document.querySelector("#tunnelPort").value) || 4000,
    public_url: document.querySelector("#publicUrl").value.trim(),
    subdomain: document.querySelector("#subdomain").value.trim() || null,
  };

  try {
    const response = await fetch("/api/tunnel/start", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    const result = await response.json();
    if (!response.ok) {
      throw new Error(result.message || "登记失败");
    }
    addLog(`公网地址已登记: ${result.url}`, "success");
    await loadTunnel();
  } catch (error) {
    addLog(String(error), "warn");
  }
});

stopTunnelButton.addEventListener("click", async () => {
  try {
    const response = await fetch("/api/tunnel/stop", { method: "POST" });
    if (!response.ok) {
      throw new Error("清空登记失败");
    }
    document.querySelector("#publicUrl").value = "";
    addLog("公网地址登记已清空", "info");
    await loadTunnel();
  } catch (error) {
    addLog(String(error), "warn");
  }
});

settingsForm.addEventListener("submit", async (event) => {
  event.preventDefault();

  const payload = {
    webhook_url: document.querySelector("#webhookUrl").value.trim() || null,
    update_rate: Number(document.querySelector("#updateRate").value) || 2000,
    auth_secret: document.querySelector("#authSecret").value.trim() || null,
    decode_depth: document.querySelector("#decodeDepth").value,
    public_hub_url: document.querySelector("#publicUrl").value.trim() || null,
    account_domain: document.querySelector("#accountDomain").value.trim() || null,
  };

  try {
    const response = await fetch("/api/settings/save", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    if (!response.ok) {
      throw new Error("保存失败");
    }
    setUpdateRate(payload.update_rate);
    addLog("系统设置已保存", "success");
  } catch (error) {
    addLog(String(error), "warn");
  }
});

workflowEditor.addEventListener("submit", async (event) => {
  event.preventDefault();

  try {
    const payload = collectWorkflowPayload();
    const response = await fetch("/api/workflows/save", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    const result = await response.json();
    if (!response.ok) {
      throw new Error(result.message || "工作流保存失败");
    }

    await loadWorkflows(payload.id);
    state.workflowDirty = false;
    addLog(`工作流定义已保存: ${payload.id}`, "success");
  } catch (error) {
    addLog(String(error), "warn");
  }
});

createWorkflowButton.addEventListener("click", () => {
  startCreateWorkflow();
});

resetWorkflowButton.addEventListener("click", () => {
  if (!confirmDiscardWorkflowChanges("重置编辑器")) {
    return;
  }

  if (state.workflowEditorMode === "new") {
    state.selectedWorkflowId = null;
    state.workflowEditorMode = "new";
    state.workflowDraft = createDefaultWorkflowDraft();
    commitWorkflowBaseline();
    renderWorkflows();
    renderWorkflowEditor();
    return;
  }

  if (state.selectedWorkflowId) {
    const workflow = state.workflows.find((item) => item.id === state.selectedWorkflowId);
    if (workflow) {
      state.workflowEditorMode = "existing";
      state.workflowDraft = cloneWorkflowDraft(workflow);
      commitWorkflowBaseline();
      renderWorkflows();
      renderWorkflowEditor();
    }
  }
});

deleteWorkflowButton.addEventListener("click", async () => {
  if (!state.selectedWorkflowId) {
    return;
  }

  if (!confirmDiscardWorkflowChanges("删除当前工作流")) {
    return;
  }

  const target = state.selectedWorkflowId;
  const confirmed = window.confirm(`确认删除工作流「${target}」吗？`);
  if (!confirmed) {
    return;
  }

  try {
    const response = await fetch(`/api/workflows/${encodeURIComponent(target)}`, {
      method: "DELETE",
    });
    const result = await response.json();
    if (!response.ok) {
      throw new Error(result.message || "工作流删除失败");
    }

    addLog(`工作流定义已删除: ${target}`, "success");
    await loadWorkflows();
  } catch (error) {
    addLog(String(error), "warn");
  }
});

workflowEditor.addEventListener("input", (event) => {
  if (event.target === workflowKindInput) {
    return;
  }
  syncWorkflowDraftFromForm();
});

workflowEditor.addEventListener("change", (event) => {
  if (event.target === workflowKindInput) {
    const nextKind = workflowKindInput.value;
    const draft = state.workflowDraft || createDefaultWorkflowDraft();
    state.workflowDraft = {
      ...draft,
      kind: nextKind,
      parameters: normalizeWorkflowParameters(nextKind, {}),
    };
    refreshWorkflowDirtyState();
    renderWorkflowEditor();
    return;
  }

  syncWorkflowDraftFromForm();
});

window.addEventListener("beforeunload", (event) => {
  if (!state.workflowDirty) {
    return;
  }

  event.preventDefault();
  event.returnValue = "";
});

refreshRunsButton.addEventListener("click", () => {
  void loadRuns(true).catch((error) => {
    addLog(String(error), "warn");
  });
});

refreshEmailsButton.addEventListener("click", () => {
  void loadEmails(true).catch((error) => {
    addLog(String(error), "warn");
  });
});

emailSearchInput.addEventListener("input", () => {
  state.emailPage.query = emailSearchInput.value;
  state.emailPage.page = 1;
  queueEmailsRefresh(300);
});

emailArchiveFilterInput.addEventListener("change", () => {
  state.emailPage.archived = emailArchiveFilterInput.value;
  state.emailPage.page = 1;
  void loadEmails(true).catch((error) => {
    addLog(String(error), "warn");
  });
});

emailPageSizeInput.addEventListener("change", () => {
  state.emailPage.pageSize = Number(emailPageSizeInput.value) || 20;
  state.emailPage.page = 1;
  void loadEmails(true).catch((error) => {
    addLog(String(error), "warn");
  });
});

clearEmailFiltersButton.addEventListener("click", () => {
  state.emailPage.query = "";
  state.emailPage.archived = "active";
  state.emailPage.pageSize = 20;
  state.emailPage.page = 1;
  emailSearchInput.value = "";
  emailArchiveFilterInput.value = "active";
  emailPageSizeInput.value = "20";
  void loadEmails(true).catch((error) => {
    addLog(String(error), "warn");
  });
});

emailViewPreviewButton.addEventListener("click", () => {
  state.emailDetailView = "preview";
  renderEmailDetailView();
});

emailViewTextButton.addEventListener("click", () => {
  state.emailDetailView = "text";
  renderEmailDetailView();
});

emailViewHtmlButton.addEventListener("click", () => {
  state.emailDetailView = "html";
  renderEmailDetailView();
});

copyEmailCodeButton.addEventListener("click", async () => {
  try {
    await copyToClipboard(state.emailDetail?.extracted_code, "验证码");
  } catch (error) {
    addLog(String(error), "warn");
  }
});

copyEmailLinkButton.addEventListener("click", async () => {
  try {
    await copyToClipboard(state.emailDetail?.extracted_link, "链接");
  } catch (error) {
    addLog(String(error), "warn");
  }
});

copyEmailTextButton.addEventListener("click", async () => {
  try {
    await copyToClipboard(state.emailDetail?.body_text, "正文");
  } catch (error) {
    addLog(String(error), "warn");
  }
});

copyEmailHtmlButton.addEventListener("click", async () => {
  try {
    await copyToClipboard(state.emailDetail?.body_html, "网页源码");
  } catch (error) {
    addLog(String(error), "warn");
  }
});

copyRunIdButton.addEventListener("click", async () => {
  try {
    await copyToClipboard(currentRun()?.id, "运行编号");
  } catch (error) {
    addLog(String(error), "warn");
  }
});

copyRunStepsButton.addEventListener("click", async () => {
  try {
    await copyToClipboard(serializeRunSteps(), "步骤日志");
  } catch (error) {
    addLog(String(error), "warn");
  }
});

copyRunAccountsButton.addEventListener("click", async () => {
  try {
    await copyToClipboard(serializeRunAccounts(), "生成账号");
  } catch (error) {
    addLog(String(error), "warn");
  }
});

exportRunSummaryButton.addEventListener("click", () => {
  try {
    const run = currentRun();
    if (!run) {
      throw new Error("当前没有选中的运行记录");
    }

    const summary = JSON.stringify(buildRunSummary(run), null, 2);
    downloadTextFile(`workflow-run-${run.id}.json`, summary, "application/json;charset=utf-8");
    addLog(`运行摘要已导出：${run.id}`, "success");
  } catch (error) {
    addLog(String(error), "warn");
  }
});

prevEmailPageButton.addEventListener("click", () => {
  if (state.emailPage.page <= 1) {
    return;
  }
  state.emailPage.page -= 1;
  void loadEmails(true).catch((error) => {
    addLog(String(error), "warn");
  });
});

nextEmailPageButton.addEventListener("click", () => {
  const totalPages = Math.max(1, Math.ceil(state.emailPage.total / state.emailPage.pageSize));
  if (state.emailPage.page >= totalPages) {
    return;
  }
  state.emailPage.page += 1;
  void loadEmails(true).catch((error) => {
    addLog(String(error), "warn");
  });
});

runStatusFilterInput.addEventListener("change", () => {
  applyRunFilters();
});

runWorkflowFilterInput.addEventListener("input", () => {
  applyRunFilters();
});

clearRunFiltersButton.addEventListener("click", () => {
  runStatusFilterInput.value = "";
  runWorkflowFilterInput.value = "";
  runWorkflowMatchInput.value = "fuzzy";
  runPageSizeInput.value = "20";
  state.runPage.pageSize = 20;
  applyRunFilters();
});

runWorkflowMatchInput.addEventListener("change", () => {
  applyRunFilters();
});

runPageSizeInput.addEventListener("change", () => {
  state.runPage.pageSize = Number(runPageSizeInput.value) || 20;
  state.runPage.page = 1;
  void loadRuns(false).catch((error) => {
    addLog(String(error), "warn");
  });
});

prevRunPageButton.addEventListener("click", () => {
  if (state.runPage.page <= 1) {
    return;
  }
  state.runPage.page -= 1;
  void loadRuns(false).catch((error) => {
    addLog(String(error), "warn");
  });
});

nextRunPageButton.addEventListener("click", () => {
  const totalPages = Math.max(1, Math.ceil(state.runPage.total / state.runPage.pageSize));
  if (state.runPage.page >= totalPages) {
    return;
  }
  state.runPage.page += 1;
  void loadRuns(false).catch((error) => {
    addLog(String(error), "warn");
  });
});

function appendOrUpdateStep(data) {
  if (!data.run_id || data.run_id !== state.selectedRunId) {
    return;
  }

  const index = state.runSteps.findIndex((item) => item.step_index === data.step_index);
  const nextStep = {
    id: `${data.run_id}-${data.step_index}`,
    run_id: data.run_id,
    step_index: data.step_index,
    level: data.level,
    message: data.msg,
    created_at: Math.floor(Date.now() / 1000),
  };

  if (index >= 0) {
    state.runSteps[index] = nextStep;
  } else {
    state.runSteps.push(nextStep);
    state.runSteps.sort((left, right) => left.step_index - right.step_index);
  }

  renderRunDetails();
}

function attachStream() {
  const eventSource = new EventSource("/stream");

  eventSource.addEventListener("new_email", (event) => {
    const data = JSON.parse(event.data);
    addLog(`新邮件流入: ${data.from} / 验证码: ${data.code || "NONE"}`, "success");
    queueEmailsRefresh(200);
  });

  eventSource.addEventListener("workflow_step", (event) => {
    const data = JSON.parse(event.data);
    appendOrUpdateStep(data);
    addLog(data.msg, data.level);
    queueRunsRefresh(250);
  });

  eventSource.addEventListener("system_log", (event) => {
    const data = JSON.parse(event.data);
    addLog(data.msg, data.level);
  });

  eventSource.onerror = () => {
    addLog("实时流断开，5 秒后自动重连", "warn");
    eventSource.close();
    setTimeout(attachStream, 5000);
  };
}

async function boot() {
  addLog("控制台初始化中...");
  await Promise.all([
    loadHealth(),
    loadEmails(),
    loadSettings(),
    loadTunnel(),
    loadWorkflows(),
    loadRuns(false),
  ]);
  attachStream();
  addLog("控制台已就绪", "success");
}

boot().catch((error) => {
  addLog(`初始化失败：${error}`, "warn");
});



