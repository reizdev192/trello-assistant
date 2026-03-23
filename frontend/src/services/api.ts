const API_BASE = '/api';

export interface TrelloCard {
  id: string;
  name: string;
  desc: string;
  idList: string;
  due: string | null;
  dueComplete: boolean | null;
  labels: Array<{
    id: string;
    name: string;
    color: string | null;
  }>;
  shortUrl: string;
  list_name: string | null;
  members: Array<{
    id: string;
    fullName: string;
    username: string;
  }>;
}

export interface ChatResponse {
  response: string;
  matched_cards: TrelloCard[];
  provider: string;
  analysis?: AnalysisData;
}

export interface AnalysisData {
  analysis_type: string;
  summary?: string;
  chart_data?: ChartDataForUI;
  insights: string[];
  time_stats?: TimeStats;
}

export interface ChartDataForUI {
  chart_type: string;
  labels: string[];
  datasets: ChartDataset[];
}

export interface ChartDataset {
  label: string;
  data: number[];
}

export interface TimeStats {
  total_est_hours: number;
  avg_hours_per_card: number;
  cards_with_est: number;
  cards_without_est: number;
  by_member: MemberTimeStat[];
  by_list: ListTimeStat[];
}

export interface MemberTimeStat {
  name: string;
  cards: number;
  hours: number;
}

export interface ListTimeStat {
  name: string;
  cards: number;
  hours: number;
}

export interface HealthResponse {
  status: string;
  ai_providers: Array<{
    name: string;
    available: boolean;
  }>;
  redis: boolean;
  trello: boolean;
}

export interface SettingsResponse {
  card_count: number;
  last_sync: string | null;
  webhooks: WebhookStatus[];
  services: {
    trello: boolean;
    redis: boolean;
    ai_providers: Array<{
      name: string;
      available: boolean;
    }>;
  };
}

export interface WebhookStatus {
  id: string;
  description: string | null;
  callback_url: string;
  active: boolean;
}

export async function sendMessage(message: string): Promise<ChatResponse> {
  const res = await fetch(`${API_BASE}/chat`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ message }),
  });

  if (!res.ok) {
    const error = await res.text();
    throw new Error(error || 'Failed to send message');
  }

  return res.json();
}

export async function refreshCards(): Promise<TrelloCard[]> {
  const res = await fetch(`${API_BASE}/cards/refresh`, { method: 'POST' });
  if (!res.ok) throw new Error('Failed to refresh cards');
  return res.json();
}

export async function getHealth(): Promise<HealthResponse> {
  const res = await fetch(`${API_BASE}/health`);
  if (!res.ok) throw new Error('Health check failed');
  return res.json();
}

export async function getSettings(): Promise<SettingsResponse> {
  const res = await fetch(`${API_BASE}/settings`);
  if (!res.ok) throw new Error('Failed to get settings');
  return res.json();
}

export async function registerWebhook(url: string): Promise<WebhookStatus> {
  const res = await fetch(`${API_BASE}/settings/webhook`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ url }),
  });
  if (!res.ok) {
    const error = await res.text();
    throw new Error(error || 'Failed to register webhook');
  }
  return res.json();
}

export async function updateWebhook(
  id: string,
  data: { description?: string; callback_url?: string; active?: boolean }
): Promise<WebhookStatus> {
  const res = await fetch(`${API_BASE}/settings/webhook/${id}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  });
  if (!res.ok) {
    const error = await res.text();
    throw new Error(error || 'Failed to update webhook');
  }
  return res.json();
}

export async function deleteWebhook(id: string): Promise<void> {
  const res = await fetch(`${API_BASE}/settings/webhook/${id}`, {
    method: 'DELETE',
  });
  if (!res.ok) {
    const error = await res.text();
    throw new Error(error || 'Failed to delete webhook');
  }
}

export interface BoardMember {
  id: string;
  full_name: string;
  username: string;
}

export async function getMembers(): Promise<BoardMember[]> {
  const res = await fetch(`${API_BASE}/members`);
  if (!res.ok) throw new Error('Failed to get members');
  return res.json();
}

export interface BoardList {
  id: string;
  name: string;
  card_count: number;
}

export interface BoardLabel {
  id: string;
  name: string;
  color: string | null;
}

export interface StatItem {
  name: string;
  count: number;
  color: string | null;
}

export interface StatsResponse {
  total_cards: number;
  overdue_count: number;
  due_soon_count: number;
  no_due_count: number;
  by_list: StatItem[];
  by_label: StatItem[];
  by_member: StatItem[];
}

export async function getLists(): Promise<BoardList[]> {
  const res = await fetch(`${API_BASE}/lists`);
  if (!res.ok) throw new Error('Failed to get lists');
  return res.json();
}

export async function getLabels(): Promise<BoardLabel[]> {
  const res = await fetch(`${API_BASE}/labels`);
  if (!res.ok) throw new Error('Failed to get labels');
  return res.json();
}

export async function getStats(): Promise<StatsResponse> {
  const res = await fetch(`${API_BASE}/stats`);
  if (!res.ok) throw new Error('Failed to get stats');
  return res.json();
}
