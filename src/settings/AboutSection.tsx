import type { AppState } from "../types";
import { SettingGroup } from "./controls";

export function AboutSection({ state }: { state: AppState }) {
  return <SettingGroup id="about" title="About" description={`Switchify PC ${state.version}`}><p className="setting-note">Switchify PC is free and open source software, licensed under the <a href="https://github.com/switchifyapp/switchify-pc/blob/main/LICENSE" target="_blank" rel="noreferrer">GNU Affero General Public License v3</a>. It comes with no warranty.</p><p className="setting-note"><a href="https://github.com/switchifyapp/switchify-pc" target="_blank" rel="noreferrer">Source code</a> · <a href="https://pc.theswitchify.app/licence/" target="_blank" rel="noreferrer">Licence and credits</a> · <a href="https://pc.theswitchify.app" target="_blank" rel="noreferrer">Online guide</a></p></SettingGroup>;
}
