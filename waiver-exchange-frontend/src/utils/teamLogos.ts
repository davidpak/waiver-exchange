const teamLogos: Record<string, string> = {
  ARI: '/teams/arizona-cardinals-logo.png',
  ATL: '/teams/atlanta-falcons-logo.png',
  BAL: '/teams/baltimore-ravens-logo.png',
  BUF: '/teams/buffalo-bills-logo.png',
  CAR: '/teams/carolina-panthers-logo.png',
  CHI: '/teams/chicago-bears-logo.png',
  CIN: '/teams/cincinnati-bengals-logo.png',
  CLE: '/teams/cleveland-browns-logo.png',
  DAL: '/teams/dallas-cowboys-logo.png',
  DEN: '/teams/denver-broncos-logo.png',
  DET: '/teams/detroit-lions-logo.png',
  GB: '/teams/green-bay-packers-logo.png',
  GBP: '/teams/green-bay-packers-logo.png',
  HOU: '/teams/houston-texans-logo.png',
  IND: '/teams/indianapolis-colts-logo.png',
  JAX: '/teams/jacksonville-jaguars-logo.png',
  JAC: '/teams/jacksonville-jaguars-logo.png',
  KC: '/teams/kansas-city-chiefs-logo.png',
  KCC: '/teams/kansas-city-chiefs-logo.png',
  LAC: '/teams/los-angeles-chargers-logo.png',
  LAR: '/teams/los-angeles-rams-logo.png',
  LV: '/teams/las-vegas-raiders-logo.png',
  LVR: '/teams/las-vegas-raiders-logo.png',
  MIA: '/teams/miami-dolphins-logo.png',
  MIN: '/teams/minnesota-vikings-logo.png',
  NE: '/teams/new-england-patriots-logo.png',
  NEP: '/teams/new-england-patriots-logo.png',
  NO: '/teams/new-orleans-saints-logo.png',
  NOR: '/teams/new-orleans-saints-logo.png',
  NYG: '/teams/new-york-giants-logo.png',
  NYJ: '/teams/new-york-jets-logo.png',
  PHI: '/teams/philadelphia-eagles-logo.png',
  PIT: '/teams/pittsburgh-steelers-logo.png',
  SEA: '/teams/seattle-seahawks-logo.png',
  SF: '/teams/san-francisco-49ers-logo.png',
  SFO: '/teams/san-francisco-49ers-logo.png',
  TB: '/teams/tampa-bay-buccaneers-logo.png',
  TBB: '/teams/tampa-bay-buccaneers-logo.png',
  TEN: '/teams/tennessee-titans-logo.png',
  WAS: '/teams/washington-commanders-logo.png',
};

const PLACEHOLDER = '/teams/placeholder-logo.jpg';

export const getTeamLogo = (teamAbbreviation: string): string => {
  return teamLogos[teamAbbreviation.toUpperCase()] || PLACEHOLDER;
};

export const getTeamLogoWithFallback = getTeamLogo;
