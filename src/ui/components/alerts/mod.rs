//! Presentation-only alerts and in-window review pages. The caller owns items
//! and handles action intents; this component never executes synchronization.
use adw::{gtk, prelude::*};
use gettextrs::gettext;
use std::rc::Rc;

type ActionHandler = Rc<dyn Fn(&str, &str)>;

/// Category controlling the summary card and its review page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AlertKind {
    Conflict,
    Problem,
}

/// A review action supplied by the presentation layer, not an engine command.
#[derive(Clone)]
pub(crate) struct AlertAction {
    pub id: String,
    pub label: String,
}

/// Display-safe item details and available adjustments. IDs must be unique
/// within this component; producers should qualify them by account.
#[derive(Clone)]
pub(crate) struct AlertItem {
    pub id: String,
    pub kind: AlertKind,
    pub title: String,
    pub detail: String,
    pub actions: Vec<AlertAction>,
}

/// Two equal-width buttons; hidden categories release their space to the other.
/// Review uses NavigationView pages in the same window, including back navigation.
pub(crate) struct Alerts {
    root: gtk::Box,
    conflicts: Category,
    problems: Category,
    on_action: ActionHandler,
}
struct Category {
    button: gtk::Button,
    count: gtk::Label,
    list: gtk::ListBox,
    empty: adw::StatusPage,
}
impl Alerts {
    /// Install on a dedicated navigation view. `on_action` receives item/action
    /// IDs on the GTK thread; the producer must revalidate before real execution.
    pub fn new(navigation: &adw::NavigationView, on_action: impl Fn(&str, &str) + 'static) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 18);
        root.set_widget_name("alerts");
        root.set_homogeneous(true);
        let css = gtk::CssProvider::new();
        css.load_from_data(include_str!("style.css"));
        gtk::style_context_add_provider_for_display(
            &root.display(),
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        root.connect_destroy(move |root| {
            gtk::style_context_remove_provider_for_display(&root.display(), &css)
        });
        let conflicts = Category::new(
            navigation,
            "alert-conflicts",
            &gettext("Pending conflicts"),
            "vcs-branch-symbolic",
            "conflicts",
        );
        let problems = Category::new(
            navigation,
            "alert-problems",
            &gettext("Problems found"),
            "dialog-warning-symbolic",
            "problems",
        );
        root.append(&conflicts.button);
        root.append(&problems.button);
        let result = Self {
            root,
            conflicts,
            problems,
            on_action: Rc::new(on_action),
        };
        result.update(&[]);
        result
    }
    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Replace counts and both review lists, including any currently open page.
    /// No callback is emitted by rendering; an empty list never implies sync success.
    pub fn update(&self, items: &[AlertItem]) {
        self.conflicts
            .update(items, AlertKind::Conflict, &self.on_action);
        self.problems
            .update(items, AlertKind::Problem, &self.on_action);
        self.root.set_visible(!items.is_empty());
    }
}
impl Category {
    fn new(
        navigation: &adw::NavigationView,
        tag: &str,
        title: &str,
        icon: &str,
        class: &str,
    ) -> Self {
        let button = gtk::Button::new();
        button.set_widget_name(tag);
        button.set_hexpand(true);
        button.add_css_class("alert-card");
        button.add_css_class(class);
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        let image = gtk::Image::from_icon_name(icon);
        image.set_pixel_size(28);
        image.add_css_class("alert-icon");
        content.append(&image);
        let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        text.set_hexpand(true);
        text.append(
            &gtk::Label::builder()
                .label(title)
                .xalign(0.0)
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .css_classes(["heading"])
                .build(),
        );
        let count = gtk::Label::builder()
            .xalign(0.0)
            .css_classes(["title-1"])
            .build();
        text.append(&count);
        content.append(&text);
        content.append(&gtk::Image::from_icon_name("go-next-symbolic"));
        button.set_child(Some(&content));
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&adw::WindowTitle::new(title, "")));
        toolbar.add_top_bar(&header);
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("boxed-list");
        let empty = adw::StatusPage::builder()
            .title(gettext("No items to review"))
            .icon_name("emblem-ok-symbolic")
            .build();
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.set_margin_top(24);
        body.set_margin_bottom(24);
        body.set_margin_start(24);
        body.set_margin_end(24);
        body.append(&list);
        body.append(&empty);
        let clamp = adw::Clamp::builder().maximum_size(800).child(&body).build();
        toolbar.set_content(Some(
            &gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Never)
                .child(&clamp)
                .build(),
        ));
        let page = adw::NavigationPage::builder()
            .title(title)
            .tag(tag)
            .child(&toolbar)
            .build();
        navigation.add(&page);
        let weak = navigation.downgrade();
        let tag = tag.to_owned();
        button.connect_clicked(move |_| {
            if let Some(nav) = weak.upgrade() {
                nav.push_by_tag(&tag);
            }
        });
        Self {
            button,
            count,
            list,
            empty,
        }
    }
    fn update(&self, items: &[AlertItem], kind: AlertKind, on_action: &ActionHandler) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        let selected: Vec<_> = items.iter().filter(|item| item.kind == kind).collect();
        self.button.set_visible(!selected.is_empty());
        self.count.set_label(&selected.len().to_string());
        self.button
            .update_property(&[gtk::accessible::Property::Label(&format!(
                "{}: {}",
                if kind == AlertKind::Conflict {
                    gettext("Pending conflicts")
                } else {
                    gettext("Problems found")
                },
                selected.len()
            ))]);
        self.list.set_visible(!selected.is_empty());
        self.empty.set_visible(selected.is_empty());
        for item in selected {
            let row = adw::ExpanderRow::builder()
                .title(&item.title)
                .subtitle(&item.detail)
                .build();
            row.set_use_markup(false);
            let actions = gtk::Box::new(gtk::Orientation::Vertical, 6);
            actions.set_margin_top(12);
            actions.set_margin_bottom(12);
            actions.set_margin_start(12);
            actions.set_margin_end(12);
            for action in &item.actions {
                let button = gtk::Button::with_label(&action.label);
                let id = item.id.clone();
                let action_id = action.id.clone();
                let callback = on_action.clone();
                button.connect_clicked(move |_| callback(&id, &action_id));
                actions.append(&button);
            }
            row.set_enable_expansion(!item.actions.is_empty());
            row.add_row(&actions);
            self.list.append(&row);
        }
    }
}

#[cfg(test)]
mod tests;
