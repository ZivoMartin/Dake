use derive_getters::Getters;

use crate::makefile::RemoteMakefile;

#[derive(Getters)]
pub struct RemoteMakefileSet {
    remote_makefiles: Vec<RemoteMakefile>,
    my_makefile: String,
}

impl RemoteMakefileSet {
    pub fn new(remote_makefiles: Vec<RemoteMakefile>, my_makefile: String) -> Self {
        Self {
            remote_makefiles,
            my_makefile,
        }
    }

    pub fn drop_makefiles(self) -> Vec<RemoteMakefile> {
        self.remote_makefiles
    }
}
