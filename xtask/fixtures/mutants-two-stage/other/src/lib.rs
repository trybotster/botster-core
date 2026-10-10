//! A second member of the fixture's workspace with a `slow` feature, which `twostage` does not depend on (like most
//! slow packages of the real workspace). So `other/slow` is valid for a workspace build only: a stage 2 that builds
//! one package with the features of every slow package fails before it tests a mutant. It has no function, so it
//! adds no mutant.
