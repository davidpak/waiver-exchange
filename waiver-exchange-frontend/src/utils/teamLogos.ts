import atlantaFalconsLogo from '@/assets/atlanta-falcons-logo.png';
import buffaloBillsLogo from '@/assets/buffalo-bills-logo.png';
import carolinaPanthersLogo from '@/assets/carolina-panthers-logo.png';
import chicagoBearsLogo from '@/assets/chicago-bears-logo.png';
import clevelandBrownsLogo from '@/assets/cleveland-browns-logo.png';
import dallasCowboysLogo from '@/assets/dallas-cowboys-logo.png';
import denverBroncosLogo from '@/assets/denver-broncos-logo.png';
import detroitLionsLogo from '@/assets/detroit-lions-logo.png';
import greenBayPackersLogo from '@/assets/green-bay-packers-logo.png';
import kansasCityChiefsLogo from '@/assets/kansas-city-chiefs-logo.png';
import miamiDolphinsLogo from '@/assets/miami-dolphins-logo.png';
import minnesotaVikingsLogo from '@/assets/minnesota-vikings-logo.png';
import newEnglandPatriotsLogo from '@/assets/new-england-patriots-logo.png';
import newOrleansSaintsLogo from '@/assets/new-orleans-saints-logo.png';
import newYorkGiantsLogo from '@/assets/new-york-giants-logo.png';
import philadelphiaEaglesLogo from '@/assets/philadelphia-eagles-logo.png';
import pittsburghSteelersLogo from '@/assets/pittsburgh-steelers-logo.png';
import placeholderLogo from '@/assets/placeholder-logo.jpg';
import seattleSeahawksLogo from '@/assets/seattle-seahawks-logo.png';
import washingtonCommandersLogo from '@/assets/washington-commanders-logo.png';

const teamLogos: Record<string, string> = {
  ATL: atlantaFalconsLogo.src,
  BUF: buffaloBillsLogo.src,
  CAR: carolinaPanthersLogo.src,
  CHI: chicagoBearsLogo.src,
  CLE: clevelandBrownsLogo.src,
  DAL: dallasCowboysLogo.src,
  DEN: denverBroncosLogo.src,
  DET: detroitLionsLogo.src,
  GB: greenBayPackersLogo.src,
  KC: kansasCityChiefsLogo.src,
  MIA: miamiDolphinsLogo.src,
  MIN: minnesotaVikingsLogo.src,
  NE: newEnglandPatriotsLogo.src,
  NO: newOrleansSaintsLogo.src,
  NYG: newYorkGiantsLogo.src,
  PHI: philadelphiaEaglesLogo.src,
  PIT: pittsburghSteelersLogo.src,
  SEA: seattleSeahawksLogo.src,
  WAS: washingtonCommandersLogo.src,
};

export const getTeamLogo = (teamAbbreviation: string): string => {
  return teamLogos[teamAbbreviation.toUpperCase()] || placeholderLogo.src;
};

export const getTeamLogoWithFallback = getTeamLogo;
