# Usage Instructions

* Begin by compiling the project itself.
* Now we must prepare the inputs for the tool. It expects a folder full of artifacts from Meh's IDA export/diff tools.
* Rename the `idaexport` folder for each version to use the version name of the executable instead (so the `idaexport` folder for the v1.10.163 release of Fallout 4 gets renamed to `1.10.163`) and place them into the same root folder.
* Collect the diff reports and rename them to indicate which versions are from the left/right columns (so the diff report between v1.10.163 and v1.10.980 would be renamed to `1.10.163_1.10.980.txt`, assuming v1.10.163 is on the left column and v1.10.980 is on the right column) and place them into the same root folder.
* Collect the version bins that have been previously released and place them in the same root folder.
* The final product should look something like:
	* `C:\libgen\artifacts\`
		* `1.10.130\`
			* `idaexport_asm.txt`
			* `idaexport_base.txt`
			* `idaexport_func.txt`
			* `idaexport_global.txt`
			* `idaexport_name.txt`
			* `idaexport_segment.txt`
			* `idaexport_string.txt`
			* `idaexport_vtable.txt`
			* `idaexport_xrefs.txt`
		* `1.10.138\`
			* `idaexport_asm.txt`
			* `idaexport_base.txt`
			* `idaexport_func.txt`
			* `idaexport_global.txt`
			* `idaexport_name.txt`
			* `idaexport_segment.txt`
			* `idaexport_string.txt`
			* `idaexport_vtable.txt`
			* `idaexport_xrefs.txt`
		* `1.10.162\`
			* `idaexport_asm.txt`
			* `idaexport_base.txt`
			* `idaexport_func.txt`
			* `idaexport_global.txt`
			* `idaexport_name.txt`
			* `idaexport_segment.txt`
			* `idaexport_string.txt`
			* `idaexport_vtable.txt`
			* `idaexport_xrefs.txt`
		* `1.10.130_1.10.138.txt`
		* `1.10.138_1.10.162.txt`
		* `version-1-10-130-0.bin`
		* `version-1-10-138-0.bin`
* Pass this root directory as an argument to the tool and wait for processing to finish.
* The tool will produce new bins in the root folder for versions which are missing them.
