mod db;

use std::{env, process, sync::Arc};

use clipboard::{ClipboardContext, ClipboardProvider};
use db::{database_parse, CategorizedEntry};
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};

use cursive::Cursive;
use cursive::CursiveExt;
use cursive::event::{Event, Key};
use cursive::traits::{Nameable, Scrollable};
use cursive::view::Resizable;
use cursive::views::{Dialog, EditView, LinearLayout, SelectView, TextView};
use cursive::views::OnEventView;

fn main() {
    let entries = database_parse();

    let args: Vec<String> = env::args().collect();
    if args.contains(&"--list".to_string()) {
        for e in &entries {
            println!("{}", e.name());
        }
        return;
    }
    if let Some((name, raw)) = parse_args(&args) {
        direct_lookup(&entries, &name, raw);
    } else {
        interactive_mode(entries);
    }
}

fn parse_args(args: &[String]) -> Option<(String, bool)> {
    if args.len() <= 1 {
        return None;
    }

    let mut raw = false;
    let mut name = None;
    for arg in &args[1..] {
        match arg.as_str() {
            "--raw" | "-r" => raw = true,
            other => name = Some(other.to_string()),
        }
    }

    name.map(|n| (n, raw))
}

fn direct_lookup(entries: &[CategorizedEntry], name: &str, raw: bool) {
    let matcher = SkimMatcherV2::default();

    let found = entries
        .iter()
        .filter_map(|e| matcher.fuzzy_match(e.name(), name).map(|score| (e, score)))
        .max_by_key(|(_, score)| *score)
        .or_else(|| {
            entries
                .iter()
                .filter_map(|e| matcher.fuzzy_match(&e.name().to_lowercase(), &name.to_lowercase()).map(|score| (e, score)))
                .max_by_key(|(_, score)| *score)
        })
        .map(|(e, _)| e);

    match found {
        Some(entry) if raw => {
            println!("{}", entry.raw_definition(entries));
        }
        Some(entry) => {
            println!("{}", entry.pretty_definition(entries));
        }
        None => {
            eprintln!("Error: no entry matching `{}` found.", name);
            process::exit(1);
        }
    }
}

fn interactive_mode(entries: Vec<CategorizedEntry>) {
    let mut siv = Cursive::default();

    let entries = Arc::new(entries);
    let matcher = Arc::new(SkimMatcherV2::default());

    let entries_clone = entries.clone();
    let entries_clone_submit = entries.clone();
    let matcher_clone = matcher.clone();
    let search_box = EditView::new()
            .on_edit(move |s, text, _| {
                update_results(s, text, &entries_clone, &matcher_clone);
            })
            .on_submit(move |siv, _text| {
                if let Some(selected) = siv
                    .call_on_name("results", |view: &mut SelectView<CategorizedEntry>| {
                        view.get_item(0).map(|(_, item)| item.clone())
                    })
                {
                    let entries = entries_clone_submit.clone();
                    siv.cb_sink()
                        .send(Box::new(move |siv| {
                            show_entry_dialog(siv, &selected.unwrap(), entries);
                        }))
                        .unwrap();
                }
            })
            .with_name("search")
            .full_width();

    let entries_clone2 = entries.clone();
    let results = SelectView::<CategorizedEntry>::new()
        .on_submit(move |s, entry| {
            show_entry_dialog(s, entry, Arc::clone(&entries_clone2));
        })
        .with_name("results")
        .full_screen();

    let layout = LinearLayout::vertical()
        .child(TextView::new("Search:"))
        .child(search_box)
        .child(results);

    let centered = Dialog::around(layout)
        .title("Fuzzy NT Docs");

    siv.add_fullscreen_layer(centered);

    update_results(&mut siv, "", &entries, &matcher);

    siv.add_global_callback(Event::Key(Key::F1), |s| show_help(s));

    siv.add_global_callback(Event::Key(Key::Esc), |s| s.quit());

    siv.run();
}

fn update_results(
    siv: &mut Cursive,
    query: &str,
    entries: &Arc<Vec<CategorizedEntry>>,
    matcher: &SkimMatcherV2,
) {
    let mut scored: Vec<(&CategorizedEntry, i64)> = if query.is_empty() {
        let mut all: Vec<_> = entries.iter().map(|e| (e, 0)).collect();
        all.sort_by_key(|(e, _)| e.name().to_string());
        all
    } else {
        let mut case_sensitive: Vec<_> = entries
            .iter()
            .filter_map(|e| matcher.fuzzy_match(e.name(), query).map(|s| (e, s)))
            .collect();
        
        if case_sensitive.is_empty() {
            entries
                .iter()
                .filter_map(|e| matcher.fuzzy_match(&e.name().to_lowercase(), &query.to_lowercase()).map(|s| (e, s)))
                .collect()
        } else {
            case_sensitive
        }
    };

    scored.sort_by(|a, b| b.1.cmp(&a.1));

    if scored.len() > 50 {
        scored.truncate(50);
    }

    siv.call_on_name("results", |view: &mut SelectView<CategorizedEntry>| {
        view.clear();
        for (entry, _) in scored {
            view.add_item(entry.name().to_string(), entry.clone());
        }
    });
}

fn show_help(s: &mut Cursive) {
    let help_text = "\
Use ↑/↓ to move the selection.
Type to filter entries via fuzzy matching.
Enter on a name opens its full definition.
Enter again on the definition copies the raw C form.
Esc backs out of dialogs or quits from the search screen.";

    let dlg = Dialog::around(TextView::new(help_text)).title("Help");
    let dlg = OnEventView::new(dlg).on_event(Key::Esc, |s| {
        s.pop_layer();
    });
    s.add_layer(dlg);
}

fn show_entry_dialog(
    s: &mut Cursive,
    entry: &CategorizedEntry,
    entries: Arc<Vec<CategorizedEntry>>,
) {
    let pretty = entry.pretty_definition(&entries);
    let raw = entry.raw_definition(&entries);
    let title = entry.name().to_string();

    let dlg = Dialog::around(TextView::new(pretty).scrollable()).title(title);
    let dlg = OnEventView::new(dlg)
        .on_event(Key::Esc, |s| {
            s.pop_layer();
        })
        .on_event(Key::Enter, move |s| {
            match ClipboardContext::new().and_then(|mut ctx| ctx.set_contents(raw.clone())) {

            Ok(_) => {
                s.add_layer(Dialog::info("Raw definition copied to clipboard"));
            }
            Err(e) => {
                eprintln!("Clipboard error: {}", e);
                s.add_layer(Dialog::info(format!("Clipboard error: {}", e)));
            }
        }

        });

    s.add_layer(dlg);
}

