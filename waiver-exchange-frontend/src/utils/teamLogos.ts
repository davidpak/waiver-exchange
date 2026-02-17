// Import team logos as static assets
import chicagoBearsLogo from '@/assets/chicago-bears-logo.png';
import dallasCowboysLogo from '@/assets/dallas-cowboys-logo.png';
import kansasCityChiefsLogo from '@/assets/kansas-city-chiefs-logo.png';
import philadelphiaEaglesLogo from '@/assets/philadelphia-eagles-logo.png';
import placeholderLogo from '@/assets/placeholder-logo.jpg';
import washingtonCommandersLogo from '@/assets/washington-commanders-logo.png';

/**
 * Maps team abbreviations to their imported logo assets
 */
export const getTeamLogo = (teamAbbreviation: string): string => {
  const teamLogos: Record<string, string> = {
    'CHI': chicagoBearsLogo.src,
    'DAL': dallasCowboysLogo.src,
    'KC': kansasCityChiefsLogo.src,
    'PHI': philadelphiaEaglesLogo.src,
    'WAS': washingtonCommandersLogo.src,
  };

  return teamLogos[teamAbbreviation.toUpperCase()] || placeholderLogo.src;
};

/**
 * Gets the team logo with fallback to placeholder
 */
export const getTeamLogoWithFallback = (teamAbbreviation: string): string => {
  const logoPath = getTeamLogo(teamAbbreviation);
  return logoPath;
};
