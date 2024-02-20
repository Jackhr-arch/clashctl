use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::prelude as Ra;
use std::{
    fs,
    fs::{remove_file, OpenOptions},
    io::Write,
    path::Path,
    rc::Rc,
};

use super::profile_input::ProfileInputPopup;
use crate::msgpopup_methods;
use crate::tui::{
    symbols::{PROFILE, TEMPALTE},
    utils::Keys,
    widgets::{ConfirmPopup, List, MsgPopup},
    EventState, SharedTheme, Visibility,
};
use crate::utils::{SharedClashTuiState, SharedClashTuiUtil};

#[derive(PartialEq)]
enum Fouce {
    Profile,
    Template,
}

#[derive(Visibility)]
pub struct ProfileTab {
    is_visible: bool,
    fouce: Fouce,

    profile_list: List,
    template_list: List,
    msgpopup: MsgPopup,
    confirm_popup: ConfirmPopup,
    profile_input: ProfileInputPopup,

    clashtui_util: SharedClashTuiUtil,
    clashtui_state: SharedClashTuiState,
}

impl ProfileTab {
    pub fn new(
        clashtui_util: SharedClashTuiUtil,
        clashtui_state: SharedClashTuiState,
        theme: SharedTheme,
    ) -> Self {
        let profiles = List::new(PROFILE.to_string(), Rc::clone(&theme));
        let templates = List::new(TEMPALTE.to_string(), Rc::clone(&theme));

        let mut instance = Self {
            is_visible: true,
            profile_list: profiles,
            template_list: templates,
            msgpopup: MsgPopup::new(),
            confirm_popup: ConfirmPopup::new(),
            fouce: Fouce::Profile,
            profile_input: ProfileInputPopup::new(),

            clashtui_util,
            clashtui_state,
        };

        instance.update_profile_list();
        instance
            .profile_list
            .select(instance.clashtui_state.borrow().get_profile());
        let template_names: Vec<String> = instance.clashtui_util.get_template_names().unwrap();
        instance.template_list.set_items(template_names);

        instance
    }

    pub fn popup_event(&mut self, ev: &Event) -> Result<EventState, ui::Infailable> {
        if !self.is_visible {
            return Ok(EventState::NotConsumed);
        }

        let mut event_state = self.msgpopup.event(ev)?;
        if event_state.is_notconsumed() {
            event_state = self.confirm_popup.event(ev)?;
        }
        if event_state.is_notconsumed() {
            event_state = self.profile_input.event(ev)?;

            if event_state == EventState::WorkDone {
                if let Event::Key(key) = ev {
                    if key.kind != KeyEventKind::Press {
                        return Ok(EventState::NotConsumed);
                    }
                    if key.code == KeyCode::Enter {
                        self.handle_import_profile_ev();
                    }
                }
            }
        }

        Ok(event_state)
    }

    fn switch_fouce(&mut self, fouce: Fouce) {
        self.fouce = fouce;
    }

    pub fn handle_select_profile_ev(&mut self) -> Option<String> {
        if let Some(profile_name) = self.profile_list.selected() {
            if let Err(err) = self.clashtui_util.select_profile(profile_name) {
                self.popup_txt_msg(err.to_string());
                None
            } else {
                Some(profile_name.to_string())
            }
        } else {
            None
        }
    }
    pub fn handle_update_profile_ev(&mut self, does_update_all: bool) {
        if let Some(profile_name) = self.profile_list.selected() {
            match self
                .clashtui_util
                .update_local_profile(profile_name, does_update_all)
            {
                Ok(res) => {
                    let mut msg = crate::utils::concat_update_profile_result(res);

                    if profile_name == self.clashtui_state.borrow().get_profile() {
                        if let Err(err) = self.clashtui_util.select_profile(profile_name) {
                            log::error!("{:?}", err);
                            msg.push(err.to_string());
                        } else {
                            msg.push("Update and selected".to_string());
                        }
                    } else {
                        msg.push("Updated".to_string());
                    }

                    self.popup_list_msg(msg);
                }
                Err(err) => {
                    self.popup_txt_msg(format!("Failed to Update: {}", err));
                }
            }
        }
    }
    pub fn handle_delete_profile_ev(&mut self) {
        if let Some(profile_name) = self.profile_list.selected() {
            match remove_file(self.clashtui_util.profile_dir.join(profile_name)) {
                Ok(()) => {
                    self.update_profile_list();
                }
                Err(err) => {
                    self.popup_txt_msg(err.to_string());
                }
            }
        }
    }

    pub fn handle_import_profile_ev(&mut self) {
        let profile_name = self.profile_input.name_input.get_input_data();
        let uri = self.profile_input.uri_input.get_input_data();
        let profile_name = profile_name.trim();
        let uri = uri.trim();

        if uri.is_empty() || profile_name.is_empty() {
            self.popup_txt_msg("Uri or Name is empty!".to_string());
            return;
        }

        if uri.starts_with("http://") || uri.starts_with("https://") {
            match OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(self.clashtui_util.profile_dir.join(profile_name))
            {
                Ok(mut file) => {
                    if let Err(err) = write!(file, "{}", uri) {
                        self.popup_txt_msg(err.to_string());
                    } else {
                        self.update_profile_list();
                    }
                }
                Err(err) => self.popup_txt_msg(err.to_string()),
            }
        } else if Path::new(uri).is_file() {
            let uri_path = Path::new(uri);
            if uri_path.exists() {
                self.popup_txt_msg("Failed to import: file exists".to_string());
                return;
            }

            let _ = fs::copy(
                Path::new(uri),
                Path::new(
                    &self
                        .clashtui_util
                        .profile_dir
                        .join(Path::new(profile_name).with_extension("yaml")),
                ),
            );
            self.update_profile_list();
        } else {
            self.popup_txt_msg("Uri is invalid.".to_string());
        }
    }

    pub fn handle_create_template_ev(&mut self) {
        if let Some(template_name) = self.template_list.selected() {
            if let Err(err) = self.clashtui_util.create_yaml_with_template(template_name) {
                self.popup_txt_msg(err.to_string());
            } else {
                self.popup_txt_msg("Created".to_string());
                self.update_profile_list();
            }
        }
    }

    pub fn update_profile_list(&mut self) {
        let profile_names: Vec<String> = self.clashtui_util.get_profile_names().unwrap();
        self.profile_list.set_items(profile_names);
    }
    pub fn event(&mut self, ev: &Event) -> Result<EventState, std::io::Error> {
        if !self.is_visible {
            return Ok(EventState::NotConsumed);
        }

        let mut event_state = EventState::NotConsumed;

        if let Event::Key(key) = ev {
            if key.kind != KeyEventKind::Press {
                return Ok(EventState::NotConsumed);
            }

            match self.fouce {
                Fouce::Profile => {
                    event_state = if Keys::TemplateSwitch.is(key) {
                        self.switch_fouce(Fouce::Template);
                        EventState::WorkDone
                    } else if Keys::Select.is(key) {
                        self.popup_txt_msg("Selecting...".to_string());
                        EventState::ProfileSelect
                    } else if Keys::ProfileUpdate.is(key) {
                        self.popup_txt_msg("Updating...".to_string());
                        EventState::ProfileUpdate
                    } else if Keys::ProfileUpdateAll.is(key) {
                        self.popup_txt_msg("Updating...".to_string());
                        EventState::ProfileUpdateAll
                    } else if Keys::ProfileImport.is(key) {
                        self.profile_input.show();
                        EventState::WorkDone
                    } else if Keys::ProfileDelete.is(key) {
                        self.confirm_popup
                            .popup_msg("`y` to Delete, `Esc` to cancel".to_string());
                        EventState::WorkDone
                    } else if Keys::Edit.is(key) {
                        if let Some(profile_name) = self.profile_list.selected() {
                            if let Err(err) = self
                                .clashtui_util
                                .edit_file(&self.clashtui_util.profile_dir.join(profile_name))
                            {
                                log::error!("{}", err);
                                self.popup_txt_msg(err.to_string());
                            }
                        }
                        EventState::WorkDone
                    } else if Keys::Preview.is(key) {
                        if let Some(profile_name) = self.profile_list.selected() {
                            let profile_path = self.clashtui_util.profile_dir.join(profile_name);
                            let file_content = std::fs::read_to_string(profile_path)?;
                            let mut lines: Vec<String> =
                                file_content.lines().map(|s| s.to_string()).collect();

                            if !self.clashtui_util.is_profile_yaml(profile_name) {
                                let yaml_path =
                                    self.clashtui_util.get_profile_yaml_path(profile_name);
                                if yaml_path.is_file() {
                                    let yaml_content = std::fs::read_to_string(&yaml_path)?;
                                    let yaml_lines: Vec<String> =
                                        yaml_content.lines().map(|s| s.to_string()).collect();
                                    lines.push(String::new());
                                    lines.extend(yaml_lines);
                                } else {
                                    lines.push(String::new());
                                    lines.push(
                                        "yaml file isn't exists. Please update it.".to_string(),
                                    );
                                }
                            }

                            self.popup_list_msg(lines);
                        }
                        EventState::WorkDone
                    } else if Keys::ProfileTestConfig.is(key) {
                        if let Some(profile_name) = self.profile_list.selected() {
                            let path = self.clashtui_util.get_profile_yaml_path(profile_name);
                            match self
                                .clashtui_util
                                .test_profile_config(path.to_str().unwrap(), false)
                            {
                                Ok(output) => {
                                    let list_msg: Vec<String> = output
                                        .lines()
                                        .map(|line| line.trim().to_string())
                                        .collect();
                                    self.popup_list_msg(list_msg);
                                }
                                Err(err) => self.popup_txt_msg(err.to_string()),
                            }
                        }
                        EventState::WorkDone
                    } else {
                        EventState::NotConsumed
                    };
                }
                Fouce::Template => {
                    event_state = if Keys::ProfileSwitch.is(key) {
                        self.switch_fouce(Fouce::Profile);
                        EventState::WorkDone
                    } else if Keys::Select.is(key) {
                        self.handle_create_template_ev();
                        EventState::WorkDone
                    } else if Keys::Preview.is(key) {
                        if let Some(name) = self.template_list.selected() {
                            let path = self
                                .clashtui_util
                                .clashtui_dir
                                .join(format!("templates/{}", name));
                            let content = std::fs::read_to_string(path)?;
                            let lines: Vec<String> =
                                content.lines().map(|s| s.to_string()).collect();

                            self.popup_list_msg(lines);
                        }
                        EventState::WorkDone
                    } else if Keys::Edit.is(key) {
                        if let Some(name) = self.template_list.selected() {
                            let tpl_file_path = self
                                .clashtui_util
                                .clashtui_dir
                                .join(format!("templates/{}", name));
                            if let Err(err) = self.clashtui_util.edit_file(&tpl_file_path) {
                                self.popup_txt_msg(err.to_string());
                            }
                        }
                        EventState::WorkDone
                    } else {
                        EventState::NotConsumed
                    };
                }
            }

            if event_state == EventState::NotConsumed {
                event_state = match self.fouce {
                    Fouce::Profile => self.profile_list.event(ev),
                    Fouce::Template => self.template_list.event(ev),
                }?;
            }
        }

        Ok(event_state)
    }

    pub fn draw(&mut self, f: &mut Ra::Frame, area: Ra::Rect) {
        if !self.is_visible() {
            return;
        }
        use Ra::{Constraint, Layout};

        let chunks = Layout::default()
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);
        let fouce = self.fouce == Fouce::Profile;
        self.profile_list.draw(f, chunks[0], fouce);
        self.template_list.draw(f, chunks[1], !fouce);

        let input_area = Layout::default()
            .constraints([
                Constraint::Percentage(25),
                Constraint::Length(8),
                Constraint::Min(0),
            ])
            .horizontal_margin(10)
            .vertical_margin(1)
            .split(f.size())[1];

        self.profile_input.draw(f, input_area);
        self.msgpopup.draw(f, area);
        self.confirm_popup.draw(f, area);
    }
}

msgpopup_methods!(ProfileTab);