//! T11.8: a fixture skill's body is absent from the prompt index and present
//! when the skill tool is called.

use std::fs;

use aulo_skills::Skills;

const BODY: &str = "The secret body line that must stay out of the prompt.";

#[test]
fn a_fixture_skill_loads_on_demand() {
    let home = tempfile::tempdir().expect("temp home");
    let skill = home.path().join("skills").join("ship-it");
    fs::create_dir_all(&skill).expect("skill dir");
    fs::write(
        skill.join("SKILL.md"),
        format!("---\nname: ship-it\ndescription: Ship the change.\n---\n{BODY}\n"),
    )
    .expect("skill file");

    let skills = Skills::load(Some(home.path()), None);
    assert!(skills.notices().is_empty(), "{:?}", skills.notices());
    let index = skills.index();
    assert!(index.contains("ship-it"));
    assert!(index.contains("Ship the change."));
    assert!(!index.contains(BODY));
    assert!(skills.spec().deferred);
    assert_eq!(skills.spec().name, "skill");

    let loaded = skills.load_body("ship-it").expect("body");
    assert!(loaded.contains(BODY));
    assert!(skills.load_body("missing").is_err());
}

#[test]
fn no_skills_leaves_the_index_empty() {
    let home = tempfile::tempdir().expect("temp home");
    let skills = Skills::load(Some(home.path()), None);
    assert_eq!(skills.index(), "");
}

#[test]
fn a_broken_skill_is_a_notice_and_the_good_one_still_loads() {
    let home = tempfile::tempdir().expect("temp home");
    let root = home.path().join("skills");
    let bad = root.join("broken");
    let good = root.join("good");
    fs::create_dir_all(&bad).expect("bad dir");
    fs::create_dir_all(&good).expect("good dir");
    fs::write(bad.join("SKILL.md"), "# no frontmatter\n").expect("bad file");
    fs::write(
        good.join("SKILL.md"),
        format!("---\nname: good\ndescription: Fine.\n---\n{BODY}\n"),
    )
    .expect("good file");

    let skills = Skills::load(Some(home.path()), None);
    assert_eq!(skills.notices().len(), 1);
    assert!(!skills.index().contains(BODY));
    assert!(skills.load_body("good").expect("good").contains(BODY));
}

#[test]
fn a_project_skill_replaces_the_home_skill() {
    let home = tempfile::tempdir().expect("temp home");
    let project = tempfile::tempdir().expect("temp project");
    for (root, desc, body) in [
        (home.path().join("skills"), "from home", "home body"),
        (
            project.path().join(".aulo").join("skills"),
            "from project",
            BODY,
        ),
    ] {
        let dir = root.join("ship-it");
        fs::create_dir_all(&dir).expect("dir");
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: ship-it\ndescription: {desc}\n---\n{body}\n"),
        )
        .expect("file");
    }
    let skills = Skills::load(Some(home.path()), Some(project.path()));
    assert!(skills.index().contains("from project"));
    assert!(!skills.index().contains("from home"));
    assert!(skills.load_body("ship-it").expect("body").contains(BODY));
    assert!(
        !skills
            .load_body("ship-it")
            .expect("body")
            .contains("home body")
    );
}
