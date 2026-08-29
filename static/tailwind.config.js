tailwind.config = {
  theme: {
    extend: {
      colors: {
        paper: '#f7f2e8', band: '#f1e7d3',
        rule: '#d9d0c0', hair: '#e3dacb', edge: '#e2d7c1', line: '#c8bda9',
        ink: '#241f1a', ink2: '#4e463d',
        muted: '#6f6659', muted2: '#6b6255', faint: '#7f7568',
        rust: '#b4531f', rustd: '#9a4620',
        pick: '#efe6d5', right: '#eae2cf', wrong: '#f6e3d9'
      },
      fontFamily: {
        serif: ['Newsreader', 'Georgia', 'serif'],
        sans: ['"Libre Franklin"', 'system-ui', 'sans-serif']
      }
    }
  },
  plugins: [
    function ({ addUtilities }) {
      addUtilities({
        '.label': { fontSize: '12px', letterSpacing: '0.12em', textTransform: 'uppercase' },
        '.label-sm': { fontSize: '11px', letterSpacing: '0.14em', textTransform: 'uppercase' }
      });
    }
  ]
};
