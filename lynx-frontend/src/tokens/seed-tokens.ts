/**
 * Daangn Seed Design Tokens (당근 시드 디자인 토큰)
 * https://seed-design.io
 */

export const SeedTokens = {
  // Brand & Semantic Colors
  color: {
    // Karrot Brand Primary
    brandPrimary: '#FF6F0F',
    brandPrimaryHover: '#FF8A3D',
    brandPrimarySubtle: 'rgba(255, 111, 15, 0.12)',

    // Semantic Positive (Profit / Buy)
    positive: '#00C471',
    positiveSubtle: 'rgba(0, 196, 113, 0.12)',

    // Semantic Negative (Loss / Sell)
    negative: '#F04452',
    negativeSubtle: 'rgba(240, 68, 82, 0.12)',

    // Semantic Warning & Info
    warning: '#FF922B',
    info: '#228BE6',

    // Grayscale (Light / Dark Mode Adaptive)
    bgDefault: '#0D1117',
    bgLayer1: '#161B22',
    bgLayer2: '#21262D',
    lineNeutral: 'rgba(240, 246, 252, 0.1)',
    lineNeutralSubtle: 'rgba(240, 246, 252, 0.05)',

    // Typography
    textPrimary: '#F0F6FC',
    textSecondary: '#8B949E',
    textMuted: '#6E7681',
    textBrand: '#FF8A3D',
  },

  // Radius (Seed Standard)
  radius: {
    xs: '4px',
    s: '8px',
    m: '12px',
    l: '16px',
    xl: '24px',
    full: '9999px',
  },

  // Elevation & Shadows
  elevation: {
    card: '0 4px 16px rgba(0, 0, 0, 0.3)',
    floating: '0 8px 24px rgba(0, 0, 0, 0.4)',
  },

  // Typography Scale
  typography: {
    fontFamily: '-apple-system, BlinkMacSystemFont, "Pretendard", "Segoe UI", sans-serif',
    titleLarge: '24px',
    titleMedium: '18px',
    titleSmall: '15px',
    bodyMedium: '14px',
    bodySmall: '12px',
    caption: '11px',
  }
};
