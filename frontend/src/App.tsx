import { useState, useRef, useEffect, useCallback } from 'react';
import {
  sendMessage,
  getSettings,
  refreshCards,
  registerWebhook,
  updateWebhook,
  deleteWebhook,
  getMembers,
  getLists,
  getLabels,
  getStats,
  type ChatResponse,
  type SettingsResponse,
  type TrelloCard,
  type BoardMember,
  type BoardList,
  type BoardLabel,
  type StatsResponse,
  type AnalysisData,
} from './services/api';
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  BarElement,
  ArcElement,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
} from 'chart.js';
import { Bar, Pie, Doughnut, Line } from 'react-chartjs-2';
import './index.css';

// Register Chart.js components
ChartJS.register(
  CategoryScale, LinearScale, BarElement, ArcElement,
  PointElement, LineElement, Title, Tooltip, Legend
);

interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  matchedCards?: TrelloCard[];
  provider?: string;
  timestamp: Date;
  analysis?: AnalysisData;
}

const INITIAL_SHOW = 8;

/* ── Card Grid Component ── */
function CardGrid({ cards }: { cards: TrelloCard[] }) {
  const [expanded, setExpanded] = useState(false);
  const visible = expanded ? cards : cards.slice(0, INITIAL_SHOW);
  const hasMore = cards.length > INITIAL_SHOW;

  return (
    <div className="card-grid">
      {visible.map(card => (
        <a
          key={card.id}
          className="card-item"
          href={card.shortUrl}
          target="_blank"
          rel="noopener noreferrer"
        >
          <div className="card-item-header">
            <span className="card-item-name">{card.name}</span>
            <span className="card-open-icon">↗</span>
          </div>
          <div className="card-item-meta">
            {card.list_name && (
              <span className="meta-tag list-tag">{card.list_name}</span>
            )}
            {card.due && (
              <span className={`meta-tag due-tag ${card.dueComplete ? 'done' : ''}`}>
                {card.dueComplete ? '✓' : '⏱'} {new Date(card.due).toLocaleDateString('vi-VN')}
              </span>
            )}
          </div>
          {card.labels.length > 0 && (
            <div className="card-labels">
              {card.labels.map(label => (
                <span key={label.id} className={`label-dot ${label.color || ''}`}>
                  {label.name || label.color}
                </span>
              ))}
            </div>
          )}
          {card.members && card.members.length > 0 && (
            <div className="card-members">
              {card.members.map(m => (
                <span key={m.id} className="member-chip" title={m.fullName}>
                  <span className="member-avatar">{m.fullName.charAt(0).toUpperCase()}</span>
                  <span className="member-name">{m.fullName}</span>
                </span>
              ))}
            </div>
          )}
        </a>
      ))}
      {hasMore && (
        <button
          className="card-grid-toggle"
          onClick={(e) => { e.stopPropagation(); setExpanded(!expanded); }}
        >
          {expanded
            ? 'Thu gọn ▲'
            : `Xem thêm ${cards.length - INITIAL_SHOW} card ▼`}
        </button>
      )}
    </div>
  );
}

/* ── Chart Color Palette ── */
const CHART_COLORS = [
  'rgba(99, 102, 241, 0.8)',   // indigo
  'rgba(16, 185, 129, 0.8)',   // emerald
  'rgba(245, 158, 11, 0.8)',   // amber
  'rgba(239, 68, 68, 0.8)',    // red
  'rgba(139, 92, 246, 0.8)',   // violet
  'rgba(6, 182, 212, 0.8)',    // cyan
  'rgba(236, 72, 153, 0.8)',   // pink
  'rgba(34, 197, 94, 0.8)',    // green
  'rgba(251, 146, 60, 0.8)',   // orange
  'rgba(59, 130, 246, 0.8)',   // blue
];
const CHART_BORDERS = CHART_COLORS.map(c => c.replace('0.8', '1'));

/* ── Analysis View Component ── */
function AnalysisView({ analysis }: { analysis: AnalysisData }) {
  return (
    <div className="analysis-view">
      {/* Summary */}
      {analysis.summary && (
        <div className="analysis-summary"
          dangerouslySetInnerHTML={{
            __html: analysis.summary
              .replace(/## (.+)/g, '<h3>$1</h3>')
              .replace(/### (.+)/g, '<h4>$1</h4>')
              .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
              .replace(/\n- /g, '<br/>• ')
              .replace(/\n/g, '<br/>')
              .replace(/\|(.+)\|/g, (match) => {
                // Basic markdown table rendering
                const rows = match.split('<br/>').filter(r => r.includes('|'));
                if (rows.length < 2) return match;
                const header = rows[0].split('|').filter(c => c.trim()).map(c => `<th>${c.trim()}</th>`).join('');
                const body = rows.slice(2).map(row => {
                  const cells = row.split('|').filter(c => c.trim()).map(c => `<td>${c.trim()}</td>`).join('');
                  return `<tr>${cells}</tr>`;
                }).join('');
                return `<table class="analysis-table"><thead><tr>${header}</tr></thead><tbody>${body}</tbody></table>`;
              })
          }}
        />
      )}

      {/* Chart */}
      {analysis.chart_data && (
        <div className="analysis-chart">
          <AnalysisChart data={analysis.chart_data} />
        </div>
      )}

      {/* Time Stats */}
      {analysis.time_stats && analysis.time_stats.total_est_hours > 0 && (
        <div className="analysis-time-stats">
          <h4>⏱ Estimated Hours</h4>
          <div className="time-stat-cards">
            <div className="time-stat-card">
              <span className="time-stat-num">{analysis.time_stats.total_est_hours}h</span>
              <span className="time-stat-label">Tổng Est</span>
            </div>
            <div className="time-stat-card">
              <span className="time-stat-num">{analysis.time_stats.avg_hours_per_card}h</span>
              <span className="time-stat-label">TB/card</span>
            </div>
            <div className="time-stat-card">
              <span className="time-stat-num">{analysis.time_stats.cards_with_est}</span>
              <span className="time-stat-label">Có Est</span>
            </div>
            <div className="time-stat-card">
              <span className="time-stat-num">{analysis.time_stats.cards_without_est}</span>
              <span className="time-stat-label">Thiếu Est</span>
            </div>
          </div>
          {analysis.time_stats.by_member.length > 0 && (
            <div className="time-member-list">
              {analysis.time_stats.by_member.slice(0, 8).map(m => (
                <div key={m.name} className="time-member-row">
                  <span className="time-member-name">{m.name}</span>
                  <span className="time-member-stats">{m.cards} cards · {m.hours}h</span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Insights */}
      {analysis.insights && analysis.insights.length > 0 && (
        <div className="analysis-insights">
          <h4>💡 Insights</h4>
          <ul>
            {analysis.insights.map((insight, i) => (
              <li key={i}>{insight}</li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

/* ── Chart Renderer ── */
function AnalysisChart({ data }: { data: { chart_type: string; labels: string[]; datasets: { label: string; data: number[] }[] } }) {
  const chartData = {
    labels: data.labels,
    datasets: data.datasets.map((ds, i) => ({
      label: ds.label,
      data: ds.data,
      backgroundColor: data.chart_type === 'bar'
        ? CHART_COLORS[i % CHART_COLORS.length]
        : CHART_COLORS.slice(0, data.labels.length),
      borderColor: data.chart_type === 'bar'
        ? CHART_BORDERS[i % CHART_BORDERS.length]
        : CHART_BORDERS.slice(0, data.labels.length),
      borderWidth: 1,
    })),
  };

  const options = {
    responsive: true,
    maintainAspectRatio: false,
    plugins: {
      legend: {
        position: 'bottom' as const,
        labels: { color: '#94a3b8', font: { size: 11 } },
      },
    },
    scales: data.chart_type === 'bar' || data.chart_type === 'line' ? {
      x: { ticks: { color: '#94a3b8', font: { size: 10 } }, grid: { color: 'rgba(148,163,184,0.1)' } },
      y: { ticks: { color: '#94a3b8', font: { size: 10 } }, grid: { color: 'rgba(148,163,184,0.1)' } },
    } : undefined,
  };

  switch (data.chart_type) {
    case 'pie': return <Pie data={chartData} options={options} />;
    case 'doughnut': return <Doughnut data={chartData} options={options} />;
    case 'line': return <Line data={chartData} options={options} />;
    default: return <Bar data={chartData} options={options} />;
  }
}

/* ── Settings Popup ── */
function SettingsPopup({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const [settings, setSettings] = useState<SettingsResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [webhookUrl, setWebhookUrl] = useState('');
  const [webhookLoading, setWebhookLoading] = useState(false);
  const [webhookMsg, setWebhookMsg] = useState('');
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editDesc, setEditDesc] = useState('');
  const [editUrl, setEditUrl] = useState('');

  const fetchSettings = useCallback(async () => {
    setLoading(true);
    try {
      const s = await getSettings();
      setSettings(s);
    } catch {
      // ignore
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (open) fetchSettings();
  }, [open, fetchSettings]);

  const handleSync = async () => {
    setSyncing(true);
    try {
      await refreshCards();
      await fetchSettings();
    } finally {
      setSyncing(false);
    }
  };

  const handleRegisterWebhook = async () => {
    if (!webhookUrl.trim()) return;
    setWebhookLoading(true);
    setWebhookMsg('');
    try {
      await registerWebhook(webhookUrl.trim());
      setWebhookMsg('✓ Đăng ký thành công');
      await fetchSettings();
    } catch (e) {
      setWebhookMsg(`✗ ${e instanceof Error ? e.message : 'Lỗi'}`);
    } finally {
      setWebhookLoading(false);
    }
  };

  if (!open) return null;

  return (
    <div className="popup-overlay" onClick={onClose}>
      <div className="popup-content" onClick={e => e.stopPropagation()}>
        <div className="popup-header">
          <h2>Cài đặt</h2>
          <button className="popup-close" onClick={onClose}>✕</button>
        </div>

        {loading ? (
          <div className="popup-loading">Đang tải...</div>
        ) : settings ? (
          <div className="popup-body">
            {/* Sync Section */}
            <div className="settings-section">
              <h3>Đồng bộ Trello</h3>
              <div className="sync-row">
                <button
                  className={`btn-sync ${syncing ? 'syncing' : ''}`}
                  onClick={handleSync}
                  disabled={syncing}
                >
                  <span className={syncing ? 'spin' : ''}>⟳</span>
                  {syncing ? 'Đang đồng bộ...' : 'Đồng bộ ngay'}
                </button>
                <div className="sync-stats">
                  <span className="stat-value">{settings.card_count} cards</span>
                  {settings.last_sync && (
                    <span className="stat-time">
                      Lần cuối: {new Date(settings.last_sync).toLocaleTimeString('vi-VN')}
                    </span>
                  )}
                </div>
              </div>
            </div>

            {/* Services Section */}
            <div className="settings-section">
              <h3>Kết nối</h3>
              <div className="service-list">
                <div className="service-row">
                  <span>Trello API</span>
                  <span className={`status-badge ${settings.services.trello ? 'online' : 'offline'}`}>
                    {settings.services.trello ? 'Online' : 'Offline'}
                  </span>
                </div>
                <div className="service-row">
                  <span>Redis Cache</span>
                  <span className={`status-badge ${settings.services.redis ? 'online' : 'offline'}`}>
                    {settings.services.redis ? 'Online' : 'Offline'}
                  </span>
                </div>
                {settings.services.ai_providers.map(p => (
                  <div key={p.name} className="service-row">
                    <span>AI: {p.name}</span>
                    <span className={`status-badge ${p.available ? 'online' : 'offline'}`}>
                      {p.available ? 'Online' : 'Offline'}
                    </span>
                  </div>
                ))}
              </div>
            </div>

            {/* Webhook Section */}
            <div className="settings-section">
              <h3>Webhooks ({settings.webhooks.length})</h3>
              {settings.webhooks.length > 0 ? (
                <div className="webhook-list">
                  {settings.webhooks.map(wh => (
                    <div key={wh.id} className="webhook-detail">
                      {editingId === wh.id ? (
                        /* ── Edit Mode ── */
                        <div className="webhook-edit-form">
                          <label>
                            <span>Mô tả</span>
                            <input
                              type="text"
                              value={editDesc}
                              onChange={e => setEditDesc(e.target.value)}
                              placeholder="Mô tả webhook"
                            />
                          </label>
                          <label>
                            <span>Callback URL</span>
                            <input
                              type="text"
                              value={editUrl}
                              onChange={e => setEditUrl(e.target.value)}
                              placeholder="https://..."
                            />
                          </label>
                          <div className="webhook-actions">
                            <button
                              className="btn-success"
                              disabled={webhookLoading}
                              onClick={async () => {
                                setWebhookLoading(true);
                                setWebhookMsg('');
                                try {
                                  await updateWebhook(wh.id, {
                                    description: editDesc,
                                    callback_url: editUrl,
                                  });
                                  setWebhookMsg('✓ Đã cập nhật webhook');
                                  setEditingId(null);
                                  await fetchSettings();
                                } catch (e) {
                                  setWebhookMsg(`✗ ${e instanceof Error ? e.message : 'Lỗi'}`);
                                } finally {
                                  setWebhookLoading(false);
                                }
                              }}
                            >
                              💾 Lưu
                            </button>
                            <button
                              className="btn-secondary"
                              onClick={() => setEditingId(null)}
                            >
                              Hủy
                            </button>
                          </div>
                        </div>
                      ) : (
                        /* ── View Mode ── */
                        <>
                          <div className="webhook-detail-rows">
                            <div className="webhook-detail-row">
                              <span className="webhook-detail-label">Trạng thái</span>
                              <span className={`status-badge ${wh.active ? 'online' : 'offline'}`}>
                                {wh.active ? '● Active' : '● Inactive'}
                              </span>
                            </div>
                            <div className="webhook-detail-row">
                              <span className="webhook-detail-label">Callback URL</span>
                              <span className="webhook-detail-value" title={wh.callback_url}>
                                {wh.callback_url}
                              </span>
                            </div>
                            {wh.description && (
                              <div className="webhook-detail-row">
                                <span className="webhook-detail-label">Mô tả</span>
                                <span className="webhook-detail-value">
                                  {wh.description}
                                </span>
                              </div>
                            )}
                            <div className="webhook-detail-row">
                              <span className="webhook-detail-label">ID</span>
                              <span className="webhook-detail-value mono">
                                {wh.id}
                              </span>
                            </div>
                          </div>
                          <div className="webhook-actions">
                            <button
                              className="btn-edit"
                              onClick={() => {
                                setEditingId(wh.id);
                                setEditDesc(wh.description || '');
                                setEditUrl(wh.callback_url);
                              }}
                            >
                              ✏️ Sửa
                            </button>
                            <button
                              className={wh.active ? 'btn-warning' : 'btn-success'}
                              disabled={webhookLoading}
                              onClick={async () => {
                                setWebhookLoading(true);
                                setWebhookMsg('');
                                try {
                                  await updateWebhook(wh.id, { active: !wh.active });
                                  setWebhookMsg(`✓ Webhook ${wh.active ? 'đã tắt' : 'đã bật'}`);
                                  await fetchSettings();
                                } catch (e) {
                                  setWebhookMsg(`✗ ${e instanceof Error ? e.message : 'Lỗi'}`);
                                } finally {
                                  setWebhookLoading(false);
                                }
                              }}
                            >
                              {wh.active ? '⏸ Tắt' : '▶ Bật'}
                            </button>
                            <button
                              className="btn-danger"
                              disabled={webhookLoading}
                              onClick={async () => {
                                if (!confirm('Xóa webhook này?')) return;
                                setWebhookLoading(true);
                                setWebhookMsg('');
                                try {
                                  await deleteWebhook(wh.id);
                                  setWebhookMsg('✓ Đã xóa webhook');
                                  await fetchSettings();
                                } catch (e) {
                                  setWebhookMsg(`✗ ${e instanceof Error ? e.message : 'Lỗi'}`);
                                } finally {
                                  setWebhookLoading(false);
                                }
                              }}
                            >
                              🗑 Xóa
                            </button>
                          </div>
                        </>
                      )}
                    </div>
                  ))}
                </div>
              ) : (
                <p style={{ fontSize: 13, color: 'var(--text-muted)' }}>Chưa có webhook nào</p>
              )}

              {/* Register new */}
              <div className="webhook-setup" style={{ marginTop: 10 }}>
                <div className="webhook-input-row">
                  <input
                    type="text"
                    placeholder="https://your-domain.com"
                    value={webhookUrl}
                    onChange={e => setWebhookUrl(e.target.value)}
                  />
                  <button
                    className="btn-primary"
                    onClick={handleRegisterWebhook}
                    disabled={webhookLoading || !webhookUrl.trim()}
                  >
                    {webhookLoading ? '...' : '+ Đăng ký'}
                  </button>
                </div>
              </div>

              {webhookMsg && (
                <div className={`webhook-msg ${webhookMsg.startsWith('✓') ? 'success' : 'error'}`}>
                  {webhookMsg}
                </div>
              )}
            </div>
          </div>
        ) : null}
      </div>
    </div>
  );
}

/* ── Dashboard Sidebar ── */
function DashboardSidebar() {
  const [stats, setStats] = useState<StatsResponse | null>(null);

  useEffect(() => {
    getStats().then(setStats).catch(() => {});
  }, []);

  const maxByList = stats ? Math.max(...stats.by_list.map(s => s.count), 1) : 1;
  const maxByMember = stats ? Math.max(...stats.by_member.map(s => s.count), 1) : 1;

  const labelColor = (c: string | null) => {
    const map: Record<string, string> = {
      green: '#61bd4f', yellow: '#f2d600', orange: '#ff9f1a', red: '#eb5a46',
      purple: '#c377e0', blue: '#0079bf', sky: '#00c2e0', lime: '#51e898',
      pink: '#ff78cb', black: '#344563', green_dark: '#519839', yellow_dark: '#d9b51c',
      orange_dark: '#cd8313', red_dark: '#b04632', purple_dark: '#89609e',
      blue_dark: '#055a8c', sky_dark: '#0098b7', lime_dark: '#4bbf6b',
      pink_dark: '#e568af', black_dark: '#091e42',
    };
    return c ? map[c] || '#8993a4' : '#8993a4';
  };

  if (!stats) {
    return (
      <aside className="sidebar">
        <div className="sidebar-loading">
          <div className="typing-dots"><span></span><span></span><span></span></div>
        </div>
      </aside>
    );
  }

  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <h2>📊 Dashboard</h2>
      </div>

      <div className="sidebar-body">
        {/* Summary */}
        <div className="stat-cards">
          <div className="stat-card">
            <span className="stat-num">{stats.total_cards}</span>
            <span className="stat-lbl">Tổng</span>
          </div>
          <div className="stat-card stat-danger">
            <span className="stat-num">{stats.overdue_count}</span>
            <span className="stat-lbl">Quá hạn</span>
          </div>
          <div className="stat-card stat-warning">
            <span className="stat-num">{stats.due_soon_count}</span>
            <span className="stat-lbl">Sắp hạn</span>
          </div>
          <div className="stat-card">
            <span className="stat-num">{stats.no_due_count}</span>
            <span className="stat-lbl">Chưa hạn</span>
          </div>
        </div>

        {/* By List */}
        <div className="sidebar-section">
          <h3>📋 Theo List</h3>
          <div className="bar-list">
            {stats.by_list.sort((a, b) => b.count - a.count).slice(0, 8).map(item => (
              <div key={item.name} className="bar-row">
                <span className="bar-name" title={item.name}>{item.name}</span>
                <div className="bar-track">
                  <div className="bar-fill" style={{ width: `${(item.count / maxByList) * 100}%` }} />
                </div>
                <span className="bar-val">{item.count}</span>
              </div>
            ))}
          </div>
        </div>

        {/* By Label */}
        <div className="sidebar-section">
          <h3>🏷️ Labels</h3>
          <div className="label-chips">
            {stats.by_label.sort((a, b) => b.count - a.count).slice(0, 12).map(item => (
              <span key={item.name} className="lbl-chip" style={{ background: labelColor(item.color) }}>
                {item.name} <b>{item.count}</b>
              </span>
            ))}
          </div>
        </div>

        {/* By Member */}
        <div className="sidebar-section">
          <h3>👥 Workload</h3>
          <div className="bar-list">
            {stats.by_member.sort((a, b) => b.count - a.count).slice(0, 8).map(item => (
              <div key={item.name} className="bar-row">
                <span className="bar-name" title={item.name}>{item.name}</span>
                <div className="bar-track">
                  <div className="bar-fill bar-blue" style={{ width: `${(item.count / maxByMember) * 100}%` }} />
                </div>
                <span className="bar-val">{item.count}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </aside>
  );
}

/* ── Slash Command Definitions ── */
const SLASH_COMMANDS = [
  { cmd: '/my', desc: 'Cards của tôi', icon: '👤' },
  { cmd: '/overdue', desc: 'Cards quá hạn', icon: '🔴' },
  { cmd: '/due', desc: 'Cards sắp hạn (VD: /due 7)', icon: '⏰' },
  { cmd: '/list', desc: 'Lọc theo list (VD: /list Doing)', icon: '📋' },
  { cmd: '/label', desc: 'Lọc theo label (VD: /label bug)', icon: '🏷️' },
  { cmd: '/stats', desc: 'Thống kê board', icon: '📊' },
  { cmd: '/recent', desc: 'Cards cập nhật gần đây', icon: '🕐' },
];

/* ── Main App ── */
function App() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState('');
  const [loading, setLoading] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Board data for filters & autocomplete
  const [allMembers, setAllMembers] = useState<BoardMember[]>([]);
  const [allLists, setAllLists] = useState<BoardList[]>([]);
  const [allLabels, setAllLabels] = useState<BoardLabel[]>([]);

  // @mention state
  const [mentionQuery, setMentionQuery] = useState<string | null>(null);
  const [mentionIndex, setMentionIndex] = useState(0);
  const [mentionStart, setMentionStart] = useState(-1);

  // #list autocomplete state
  const [hashQuery, setHashQuery] = useState<string | null>(null);
  const [hashIndex, setHashIndex] = useState(0);
  const [hashStart, setHashStart] = useState(-1);

  // /slash command state
  const [slashQuery, setSlashQuery] = useState<string | null>(null);
  const [slashIndex, setSlashIndex] = useState(0);

  // Substitution map: display name → identifier (for precise backend matching)
  // e.g. { "@Xuân Phạm": "@xuanpham", "#Task list - DEV": "#list_id_123" }
  const mentionSubs = useRef<Record<string, string>>({});

  // Quick filter state
  const [filterList, setFilterList] = useState<string | null>(null);
  const [filterMember, setFilterMember] = useState<string | null>(null);
  const [filterLabel, setFilterLabel] = useState<string | null>(null);
  const [filterDue, setFilterDue] = useState<string | null>(null);
  const [filterDropdown, setFilterDropdown] = useState<string | null>(null);

  const filteredMembers = mentionQuery !== null
    ? allMembers.filter(m =>
        m.full_name.toLowerCase().includes(mentionQuery.toLowerCase()) ||
        m.username.toLowerCase().includes(mentionQuery.toLowerCase())
      ).slice(0, 8)
    : [];

  const filteredLists = hashQuery !== null
    ? allLists.filter(l =>
        l.name.toLowerCase().includes(hashQuery.toLowerCase())
      ).slice(0, 10)
    : [];

  const filteredSlash = slashQuery !== null
    ? SLASH_COMMANDS.filter(c =>
        c.cmd.toLowerCase().startsWith(`/${slashQuery.toLowerCase()}`)
      )
    : [];

  // Load data on mount
  useEffect(() => {
    getMembers().then(setAllMembers).catch(() => {});
    getLists().then(setAllLists).catch(() => {});
    getLabels().then(setAllLabels).catch(() => {});
  }, []);

  const scrollToBottom = useCallback(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, []);

  useEffect(() => {
    scrollToBottom();
  }, [messages, scrollToBottom]);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Detect @, # and / in input
  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    setInput(value);

    const cursorPos = e.target.selectionStart || value.length;
    const textBeforeCursor = value.slice(0, cursorPos);

    // Check / slash command (only at start)
    if (textBeforeCursor.startsWith('/')) {
      const query = textBeforeCursor.slice(1);
      setSlashQuery(query);
      setSlashIndex(0);
      setMentionQuery(null);
      setHashQuery(null);
      return;
    }
    setSlashQuery(null);

    // Check # list autocomplete
    const hashIdx = textBeforeCursor.lastIndexOf('#');
    if (hashIdx >= 0 && (hashIdx === 0 || textBeforeCursor[hashIdx - 1] === ' ')) {
      const query = textBeforeCursor.slice(hashIdx + 1);
      if (!query.includes(' ') || query.length <= 20) {
        setHashQuery(query);
        setHashStart(hashIdx);
        setHashIndex(0);
        setMentionQuery(null);
        return;
      }
    }
    setHashQuery(null);

    // Check @ member mention
    const atIndex = textBeforeCursor.lastIndexOf('@');
    if (atIndex >= 0 && (atIndex === 0 || textBeforeCursor[atIndex - 1] === ' ')) {
      const query = textBeforeCursor.slice(atIndex + 1);
      if (!query.includes(' ') || query.length <= 20) {
        setMentionQuery(query);
        setMentionStart(atIndex);
        setMentionIndex(0);
        return;
      }
    }
    setMentionQuery(null);
  };

  // Select a member from dropdown — show full_name in input, map to username
  const selectMember = (member: BoardMember) => {
    const before = input.slice(0, mentionStart);
    const afterCursor = input.slice(mentionStart + 1 + (mentionQuery?.length || 0));
    // Show readable name in input
    setInput(`${before}@${member.full_name}${afterCursor ? afterCursor : ' '}`);
    // Track substitution: display → backend identifier
    mentionSubs.current[`@${member.full_name}`] = `@${member.username}`;
    setMentionQuery(null);
    setMentionStart(-1);
    inputRef.current?.focus();
  };

  // Select a list from # dropdown — show list name in input, map to list ID
  const selectList = (list: BoardList) => {
    const before = input.slice(0, hashStart);
    const afterCursor = input.slice(hashStart + 1 + (hashQuery?.length || 0));
    // Show readable name in input
    setInput(`${before}#${list.name}${afterCursor ? afterCursor : ' '}`);
    // Track substitution: display → backend identifier
    mentionSubs.current[`#${list.name}`] = `#${list.id}`;
    setHashQuery(null);
    setHashStart(-1);
    inputRef.current?.focus();
  };

  // Select a slash command
  const selectSlashCommand = (cmd: string) => {
    setInput(cmd + ' ');
    setSlashQuery(null);
    inputRef.current?.focus();
  };

  // Execute quick filter
  const executeFilter = async () => {
    if (!filterList && !filterMember && !filterLabel && !filterDue) return;
    setLoading(true);

    const displayParts: string[] = [];  // For chat message (human readable)
    const apiParts: string[] = [];      // For API (IDs/usernames)

    if (filterList) {
      const listObj = allLists.find(l => l.name === filterList);
      displayParts.push(`#${filterList}`);
      apiParts.push(`#${listObj ? listObj.id : filterList}`);
    }
    if (filterMember) {
      const memberObj = allMembers.find(m => m.full_name === filterMember);
      displayParts.push(`@${filterMember}`);
      apiParts.push(`@${memberObj ? memberObj.username : filterMember}`);
    }
    if (filterLabel) {
      displayParts.push(`label:${filterLabel}`);
      apiParts.push(`label:${filterLabel}`);
    }
    if (filterDue === 'overdue') { displayParts.push('quá hạn'); apiParts.push('quá hạn'); }
    else if (filterDue === 'due_soon') { displayParts.push('sắp hạn'); apiParts.push('sắp hạn'); }
    else if (filterDue === 'no_due') { displayParts.push('chưa có deadline'); apiParts.push('chưa có deadline'); }

    const userMsg: Message = {
      id: crypto.randomUUID(), role: 'user',
      content: `🔍 Filter: ${displayParts.join(' ')}`, timestamp: new Date(),
    };
    setMessages(prev => [...prev, userMsg]);

    try {
      const response: ChatResponse = await sendMessage(apiParts.join(' '));
      setMessages(prev => [...prev, {
        id: crypto.randomUUID(), role: 'assistant',
        content: response.response, matchedCards: response.matched_cards,
        provider: response.provider, timestamp: new Date(),
        analysis: response.analysis,
      }]);
    } catch (error) {
      setMessages(prev => [...prev, {
        id: crypto.randomUUID(), role: 'assistant',
        content: `Lỗi: ${error instanceof Error ? error.message : 'Lỗi'}`,
        timestamp: new Date(),
      }]);
    } finally {
      setLoading(false);
    }
  };

  const clearAllFilters = () => {
    setFilterList(null); setFilterMember(null);
    setFilterLabel(null); setFilterDue(null);
    setFilterDropdown(null);
  };

  const hasActiveFilter = filterList || filterMember || filterLabel || filterDue;

  // Resolve display text → API identifiers before sending
  const resolveMessageForApi = (displayText: string): string => {
    let resolved = displayText;
    // Apply all tracked substitutions (longest first to avoid partial matches)
    const subs = Object.entries(mentionSubs.current)
      .sort(([a], [b]) => b.length - a.length);
    for (const [display, identifier] of subs) {
      resolved = resolved.replace(display, identifier);
    }
    return resolved;
  };

  const handleSend = async (text?: string) => {
    const displayText = text || input.trim();
    if (!displayText || loading) return;

    setMentionQuery(null);

    // User sees the display text (with names)
    const userMessage: Message = {
      id: crypto.randomUUID(),
      role: 'user',
      content: displayText,
      timestamp: new Date(),
    };

    setMessages(prev => [...prev, userMessage]);
    setInput('');
    setLoading(true);

    // API receives resolved text (with IDs/usernames)
    const apiMessage = resolveMessageForApi(displayText);

    try {
      const response: ChatResponse = await sendMessage(apiMessage);
      const assistantMessage: Message = {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: response.response,
        matchedCards: response.matched_cards,
        provider: response.provider,
        timestamp: new Date(),
        analysis: response.analysis,
      };
      setMessages(prev => [...prev, assistantMessage]);
    } catch (error) {
      const errorMessage: Message = {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: `Lỗi: ${error instanceof Error ? error.message : 'Không kết nối được server'}`,
        timestamp: new Date(),
      };
      setMessages(prev => [...prev, errorMessage]);
    } finally {
      setLoading(false);
      inputRef.current?.focus();
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    // Slash command dropdown
    if (slashQuery !== null && filteredSlash.length > 0) {
      if (e.key === 'ArrowDown') { e.preventDefault(); setSlashIndex(i => (i + 1) % filteredSlash.length); return; }
      if (e.key === 'ArrowUp') { e.preventDefault(); setSlashIndex(i => (i - 1 + filteredSlash.length) % filteredSlash.length); return; }
      if (e.key === 'Enter' || e.key === 'Tab') { e.preventDefault(); selectSlashCommand(filteredSlash[slashIndex].cmd); return; }
      if (e.key === 'Escape') { e.preventDefault(); setSlashQuery(null); return; }
    }

    // # list dropdown
    if (hashQuery !== null && filteredLists.length > 0) {
      if (e.key === 'ArrowDown') { e.preventDefault(); setHashIndex(i => (i + 1) % filteredLists.length); return; }
      if (e.key === 'ArrowUp') { e.preventDefault(); setHashIndex(i => (i - 1 + filteredLists.length) % filteredLists.length); return; }
      if (e.key === 'Enter' || e.key === 'Tab') { e.preventDefault(); selectList(filteredLists[hashIndex]); return; }
      if (e.key === 'Escape') { e.preventDefault(); setHashQuery(null); return; }
    }

    // @mention dropdown
    if (mentionQuery !== null && filteredMembers.length > 0) {
      if (e.key === 'ArrowDown') { e.preventDefault(); setMentionIndex(i => (i + 1) % filteredMembers.length); return; }
      if (e.key === 'ArrowUp') { e.preventDefault(); setMentionIndex(i => (i - 1 + filteredMembers.length) % filteredMembers.length); return; }
      if (e.key === 'Enter' || e.key === 'Tab') { e.preventDefault(); selectMember(filteredMembers[mentionIndex]); return; }
      if (e.key === 'Escape') { e.preventDefault(); setMentionQuery(null); return; }
    }

    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="app">
      {/* ── Left: Chat Panel ── */}
      <div className="chat-panel">
        {/* Top Bar */}
        <header className="topbar">
          <div className="topbar-left">
            <span className="topbar-logo">📋</span>
            <span className="topbar-title">Trello Assistant</span>
          </div>
          <div className="topbar-right">
            <button className="topbar-btn" onClick={() => setSettingsOpen(true)} title="Cài đặt">
              ⚙
            </button>
          </div>
        </header>

        {/* Quick Filters */}
        <div className="filter-bar">
          <div className="filter-chips">
            {/* List filter */}
            <div className="filter-chip-wrapper">
              <button className={`filter-chip ${filterList ? 'active' : ''}`}
                onClick={() => setFilterDropdown(filterDropdown === 'list' ? null : 'list')}>
                📋 {filterList || 'List'} ▾
              </button>
              {filterDropdown === 'list' && (
                <div className="filter-dropdown">
                  {allLists.map(l => (
                    <button key={l.id} className={`filter-option ${filterList === l.name ? 'selected' : ''}`}
                      onClick={() => { setFilterList(filterList === l.name ? null : l.name); setFilterDropdown(null); }}>
                      {l.name} <span className="filter-count">{l.card_count}</span>
                    </button>
                  ))}
                </div>
              )}
            </div>

            {/* Member filter */}
            <div className="filter-chip-wrapper">
              <button className={`filter-chip ${filterMember ? 'active' : ''}`}
                onClick={() => setFilterDropdown(filterDropdown === 'member' ? null : 'member')}>
                👤 {filterMember || 'Member'} ▾
              </button>
              {filterDropdown === 'member' && (
                <div className="filter-dropdown">
                  {allMembers.map(m => (
                    <button key={m.id} className={`filter-option ${filterMember === m.full_name ? 'selected' : ''}`}
                      onClick={() => { setFilterMember(filterMember === m.full_name ? null : m.full_name); setFilterDropdown(null); }}>
                      {m.full_name}
                    </button>
                  ))}
                </div>
              )}
            </div>

            {/* Label filter */}
            <div className="filter-chip-wrapper">
              <button className={`filter-chip ${filterLabel ? 'active' : ''}`}
                onClick={() => setFilterDropdown(filterDropdown === 'label' ? null : 'label')}>
                🏷️ {filterLabel || 'Label'} ▾
              </button>
              {filterDropdown === 'label' && (
                <div className="filter-dropdown">
                  {allLabels.map(l => (
                    <button key={l.id} className={`filter-option ${filterLabel === l.name ? 'selected' : ''}`}
                      onClick={() => { setFilterLabel(filterLabel === l.name ? null : l.name); setFilterDropdown(null); }}>
                      {l.name}
                    </button>
                  ))}
                </div>
              )}
            </div>

            {/* Due filter */}
            <div className="filter-chip-wrapper">
              <button className={`filter-chip ${filterDue ? 'active' : ''}`}
                onClick={() => setFilterDropdown(filterDropdown === 'due' ? null : 'due')}>
                ⏰ {filterDue === 'overdue' ? 'Quá hạn' : filterDue === 'due_soon' ? 'Sắp hạn' : filterDue === 'no_due' ? 'Chưa hạn' : 'Deadline'} ▾
              </button>
              {filterDropdown === 'due' && (
                <div className="filter-dropdown">
                  <button className={`filter-option ${filterDue === 'overdue' ? 'selected' : ''}`}
                    onClick={() => { setFilterDue(filterDue === 'overdue' ? null : 'overdue'); setFilterDropdown(null); }}>🔴 Quá hạn</button>
                  <button className={`filter-option ${filterDue === 'due_soon' ? 'selected' : ''}`}
                    onClick={() => { setFilterDue(filterDue === 'due_soon' ? null : 'due_soon'); setFilterDropdown(null); }}>🟡 Sắp hạn (7 ngày)</button>
                  <button className={`filter-option ${filterDue === 'no_due' ? 'selected' : ''}`}
                    onClick={() => { setFilterDue(filterDue === 'no_due' ? null : 'no_due'); setFilterDropdown(null); }}>⚪ Chưa có deadline</button>
                </div>
              )}
            </div>
          </div>

          {hasActiveFilter && (
            <div className="filter-actions">
              <button className="filter-apply" onClick={executeFilter} disabled={loading}>
                🔍 Lọc
              </button>
              <button className="filter-clear" onClick={clearAllFilters}>✕</button>
            </div>
          )}
        </div>

        {/* Messages */}
        <main className="messages-area">
          {messages.length === 0 ? (
            <div className="empty-state">
              <div className="empty-icon">💬</div>
              <p>Tìm kiếm card Trello...</p>
              <div className="empty-hints">
                <span>Gõ <kbd>@</kbd> tag member</span>
                <span>Gõ <kbd>#</kbd> chọn list</span>
                <span>Gõ <kbd>/</kbd> xem commands</span>
              </div>
            </div>
          ) : (
            messages.map(msg => (
              <div key={msg.id} className={`message ${msg.role}`}>
                <div className="message-bubble">
                  <span dangerouslySetInnerHTML={{
                    __html: msg.content
                      .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
                      .replace(/@(\S+)/g, '<span class="mention-tag">@$1</span>')
                      .replace(/#(\S+)/g, '<span class="list-ref-tag">#$1</span>')
                      .replace(/\n/g, '<br/>')
                  }} />
                </div>

                {msg.analysis && (
                  <AnalysisView analysis={msg.analysis} />
                )}

                {msg.matchedCards && msg.matchedCards.length > 0 && (
                  <CardGrid cards={msg.matchedCards} />
                )}

                <div className="message-time">
                  {msg.timestamp.toLocaleTimeString('vi-VN')}
                  {msg.provider && <span> · {msg.provider}</span>}
                </div>
              </div>
            ))
          )}

          {loading && (
            <div className="message assistant">
              <div className="message-bubble">
                <div className="typing-dots">
                  <span></span><span></span><span></span>
                </div>
              </div>
            </div>
          )}

          <div ref={messagesEndRef} />
        </main>

        {/* Input */}
        <footer className="input-bar">
          <div className="input-wrapper">
            {/* / Slash command dropdown */}
            {slashQuery !== null && filteredSlash.length > 0 && (
              <div className="mention-dropdown">
                {filteredSlash.map((c, i) => (
                  <button key={c.cmd}
                    className={`mention-item ${i === slashIndex ? 'active' : ''}`}
                    onClick={() => selectSlashCommand(c.cmd)}
                    onMouseEnter={() => setSlashIndex(i)}>
                    <span className="mention-avatar">{c.icon}</span>
                    <span className="mention-info">
                      <span className="mention-name">{c.cmd}</span>
                      <span className="mention-username">{c.desc}</span>
                    </span>
                  </button>
                ))}
              </div>
            )}

            {/* # List dropdown */}
            {hashQuery !== null && filteredLists.length > 0 && (
              <div className="mention-dropdown">
                {filteredLists.map((l, i) => (
                  <button key={l.id}
                    className={`mention-item ${i === hashIndex ? 'active' : ''}`}
                    onClick={() => selectList(l)}
                    onMouseEnter={() => setHashIndex(i)}>
                    <span className="mention-avatar">📋</span>
                    <span className="mention-info">
                      <span className="mention-name">{l.name}</span>
                      <span className="mention-username">{l.card_count} cards</span>
                    </span>
                  </button>
                ))}
              </div>
            )}

            {/* @mention dropdown */}
            {mentionQuery !== null && filteredMembers.length > 0 && (
              <div className="mention-dropdown">
                {filteredMembers.map((m, i) => (
                  <button key={m.id}
                    className={`mention-item ${i === mentionIndex ? 'active' : ''}`}
                    onClick={() => selectMember(m)}
                    onMouseEnter={() => setMentionIndex(i)}>
                    <span className="mention-avatar">{m.full_name.charAt(0).toUpperCase()}</span>
                    <span className="mention-info">
                      <span className="mention-name">{m.full_name}</span>
                      <span className="mention-username">@{m.username}</span>
                    </span>
                  </button>
                ))}
              </div>
            )}

            <div className="input-row">
              <input
                ref={inputRef}
                type="text"
                placeholder="Tìm card... ( @ member · # list · / command )"
                value={input}
                onChange={handleInputChange}
                onKeyDown={handleKeyDown}
                disabled={loading}
                autoFocus
              />
              <button
                className="btn-send"
                onClick={() => handleSend()}
                disabled={!input.trim() || loading}
              >
                ➤
              </button>
            </div>
          </div>
        </footer>
      </div>

      {/* ── Right: Dashboard Sidebar ── */}
      <DashboardSidebar />

      {/* Popups */}
      <SettingsPopup open={settingsOpen} onClose={() => setSettingsOpen(false)} />
    </div>
  );
}

export default App;
