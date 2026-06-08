const fs = require('node:fs');
const path = require('node:path');
const ResEdit = require('resedit');

module.exports = async function applyWindowsIcon(context) {
  if (context.electronPlatformName !== 'win32') {
    return;
  }

  const executableName = `${context.packager.appInfo.productFilename}.exe`;
  const executablePath = path.join(context.appOutDir, executableName);
  const iconPath = path.join(context.packager.info.projectDir, 'build', 'icon.ico');

  const executable = ResEdit.NtExecutable.from(fs.readFileSync(executablePath), { ignoreCert: true });
  const resources = ResEdit.NtExecutableResource.from(executable);
  const iconFile = ResEdit.Data.IconFile.from(fs.readFileSync(iconPath));

  ResEdit.Resource.IconGroupEntry.replaceIconsForResource(
    resources.entries,
    1,
    1033,
    iconFile.icons.map((item) => item.data)
  );

  resources.outputResource(executable);
  fs.writeFileSync(executablePath, Buffer.from(executable.generate()));
};
