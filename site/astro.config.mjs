import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://pc.theswitchify.app',
  integrations: [
    starlight({
      title: 'Switchify PC',
      description: 'Switch access for Windows and Mac. How to set up Switchify PC, scan, type, and pair it with Switchify on Android.',
      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/switchifyapp/switchify-pc' },
      ],
      editLink: {
        baseUrl: 'https://github.com/switchifyapp/switchify-pc/edit/main/site/',
      },
      lastUpdated: true,
      customCss: ['./src/styles/custom.css'],
      sidebar: [
        { label: 'Overview', slug: 'index' },
        {
          label: 'Get started',
          items: [
            { label: 'Install', slug: 'get-started/install' },
            { label: 'Add a switch', slug: 'get-started/add-a-switch' },
          ],
        },
        {
          label: 'Using the computer',
          items: [
            { label: 'Home menu', slug: 'using/home-menu' },
            { label: 'Point scanning', slug: 'using/point-scanning' },
            { label: 'Mouse panel and radar', slug: 'using/mouse-panel' },
            { label: 'Keyboard', slug: 'using/keyboard' },
            { label: 'Word prediction', slug: 'using/word-prediction' },
            { label: 'Scanning settings', slug: 'using/scanning-settings' },
          ],
        },
        {
          label: 'With Switchify on Android',
          items: [
            { label: 'Pair your device', slug: 'android/pairing' },
            { label: 'PC control', slug: 'android/pc-control' },
            { label: 'Switch Forwarding', slug: 'android/switch-forwarding' },
          ],
        },
        { label: 'Account, updates and privacy', slug: 'account-and-privacy' },
        { label: 'Troubleshooting', slug: 'troubleshooting' },
        { label: 'Release notes', link: 'https://github.com/switchifyapp/switchify-pc/releases' },
      ],
    }),
  ],
});
