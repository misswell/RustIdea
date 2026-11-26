use std::path::{Path, PathBuf};

use ide_build::{BuildModel, DependencyCoordinate};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildModuleRow {
    pub label: String,
    pub detail: String,
    pub path: PathBuf,
}

pub fn module_rows(root: &Path, model: &BuildModel) -> Vec<BuildModuleRow> {
    let mut rows = Vec::new();
    if let Some(maven) = &model.maven {
        for project in &maven.projects {
            rows.push(BuildModuleRow {
                label: project.display_name.clone(),
                detail: format!(
                    "Maven · {}:{}:{}",
                    project.group_id, project.artifact_id, project.version
                ),
                path: root.join(&project.pom),
            });
        }
    }
    for build in &model.gradle {
        for project in &build.projects {
            let build_dir = root.join(&build.dir);
            let path = match &project.build_file {
                Some(file) => build_dir.join(&project.dir).join(file),
                None => build_dir.join(&project.dir),
            };
            rows.push(BuildModuleRow {
                label: project.name.clone(),
                detail: format!("Gradle · {}", project.path),
                path,
            });
        }
    }
    for build in &model.sbt {
        let build_dir = root.join(&build.dir);
        for project in &build.projects {
            rows.push(BuildModuleRow {
                label: project.id.clone(),
                detail: format!(
                    "sbt · {}{}",
                    if project.root {
                        "root project"
                    } else {
                        "project"
                    },
                    build
                        .version
                        .as_ref()
                        .map(|version| format!(" · sbt {version}"))
                        .unwrap_or_default()
                ),
                path: build_dir.join("build.sbt"),
            });
        }
    }
    rows
}

pub fn dependency_rows(root: &Path, model: &BuildModel) -> Vec<DependencyCoordinate> {
    ide_build::dependency_coordinates(root, model)
}

pub fn coordinate_label(coordinate: &DependencyCoordinate) -> String {
    format!(
        "{}:{}:{}",
        coordinate.group_id, coordinate.artifact_id, coordinate.version
    )
}

pub fn source_label(coordinate: &DependencyCoordinate) -> &'static str {
    if coordinate.gradle { "Gradle" } else { "Maven" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ide_build::{
        MavenDependency, MavenModel, MavenProject, gradle::GradleModel, gradle::GradleProject,
    };

    fn maven_project() -> MavenProject {
        MavenProject {
            pom: "pom.xml".into(),
            dir: String::new(),
            group_id: "com.example".into(),
            artifact_id: "app".into(),
            version: "1.0".into(),
            display_name: "App".into(),
            dependencies: vec![MavenDependency {
                group_id: "org.junit".into(),
                artifact_id: "junit".into(),
                version: Some("4.13".into()),
                ..MavenDependency::default()
            }],
            ..MavenProject::default()
        }
    }

    #[test]
    fn lists_maven_gradle_and_sbt_modules() {
        let model = BuildModel {
            maven: Some(MavenModel {
                projects: vec![maven_project()],
                ..MavenModel::default()
            }),
            gradle: vec![GradleModel {
                dir: "gradle-build".into(),
                projects: vec![GradleProject {
                    path: ":app".into(),
                    name: "app".into(),
                    dir: "app".into(),
                    build_file: Some("build.gradle.kts".into()),
                    ..GradleProject::default()
                }],
                ..GradleModel::default()
            }],
            sbt: vec![ide_build::sbt::SbtBuild {
                dir: String::new(),
                command: "sbt".into(),
                version: Some("1.9".into()),
                projects: vec![ide_build::sbt::SbtProject {
                    id: "root".into(),
                    dir: String::new(),
                    root: true,
                }],
            }],
        };
        let rows = module_rows(Path::new("/repo"), &model);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].path, Path::new("/repo/pom.xml"));
        assert!(rows[1].detail.contains("Gradle · :app"));
        assert_eq!(rows[2].path, Path::new("/repo/build.sbt"));
    }

    #[test]
    fn formats_dependency_source_and_coordinate() {
        let maven = DependencyCoordinate {
            gradle: false,
            group_id: "org.junit".into(),
            artifact_id: "junit".into(),
            version: "4.13".into(),
        };
        assert_eq!(coordinate_label(&maven), "org.junit:junit:4.13");
        assert_eq!(source_label(&maven), "Maven");
        let gradle = DependencyCoordinate {
            gradle: true,
            ..maven
        };
        assert_eq!(source_label(&gradle), "Gradle");
    }
}
