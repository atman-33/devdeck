export interface Project {
  path: string;
  name: string;
  tags: string;
  favorite: boolean;
  notes: string;
  last_opened: number | null;
}

export interface Preset {
  name: string;
  paths: string[];
}

export interface Settings {
  vscode_cmd: string;
  terminal_cmd: string;
  agent_cmd: string;
  check_updates: boolean;
}

export type SortMode = "Name" | "Recent";

export interface Config {
  version: number;
  projects: Project[];
  presets: Preset[];
  selected: string[];
  settings: Settings;
  sort: SortMode;
}

export interface GitInfo {
  is_repo: boolean;
  branch: string;
  detached: boolean;
  has_upstream: boolean;
  ahead: number;
  behind: number;
  changes: number;
  branches: string[];
  error: string | null;
}

export interface UpdateInfo {
  tag: string;
  url: string;
}
