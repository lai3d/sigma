import { useState, useRef, memo } from 'react';
import {
  ComposableMap,
  Geographies,
  Geography,
} from 'react-simple-maps';
import { COUNTRIES } from '@/lib/countries';

const GEO_URL = 'https://cdn.jsdelivr.net/npm/world-atlas@2/countries-110m.json';

// ISO 3166-1 numeric-3 → alpha-2 mapping
const NUM_TO_A2: Record<string, string> = {
  // Our COUNTRIES list
  '036': 'AU', '076': 'BR', '124': 'CA', '156': 'CN', '158': 'TW',
  '250': 'FR', '276': 'DE', '344': 'HK', '356': 'IN', '360': 'ID',
  '392': 'JP', '410': 'KR', '446': 'MO', '458': 'MY', '528': 'NL',
  '608': 'PH', '643': 'RU', '702': 'SG', '756': 'CH', '764': 'TH',
  '792': 'TR', '826': 'GB', '840': 'US', '704': 'VN',
  // Europe
  '040': 'AT', '056': 'BE', '100': 'BG', '191': 'HR', '196': 'CY',
  '203': 'CZ', '208': 'DK', '233': 'EE', '246': 'FI', '300': 'GR',
  '348': 'HU', '372': 'IE', '380': 'IT', '428': 'LV', '440': 'LT',
  '442': 'LU', '470': 'MT', '616': 'PL', '620': 'PT', '642': 'RO',
  '703': 'SK', '705': 'SI', '724': 'ES', '752': 'SE', '578': 'NO',
  '352': 'IS', '804': 'UA', '112': 'BY', '498': 'MD', '688': 'RS',
  '499': 'ME', '008': 'AL', '807': 'MK', '070': 'BA',
  // Americas
  '032': 'AR', '152': 'CL', '170': 'CO', '218': 'EC', '604': 'PE',
  '484': 'MX', '858': 'UY', '862': 'VE', '591': 'PA', '188': 'CR',
  // Africa & Middle East
  '710': 'ZA', '566': 'NG', '404': 'KE', '818': 'EG',
  '784': 'AE', '682': 'SA', '376': 'IL', '634': 'QA', '414': 'KW',
  // Asia & Oceania
  '554': 'NZ', '586': 'PK', '050': 'BD', '116': 'KH', '418': 'LA',
  '104': 'MM', '524': 'NP', '398': 'KZ', '860': 'UZ',
  '268': 'GE', '051': 'AM', '031': 'AZ', '496': 'MN',
};

// Build name lookup from our COUNTRIES list
const CODE_TO_NAME: Record<string, string> = {};
COUNTRIES.forEach((c) => {
  CODE_TO_NAME[c.code] = c.name;
});

function getCountryName(alpha2: string): string {
  return CODE_TO_NAME[alpha2] || alpha2;
}

interface Props {
  countryData: { name: string; value: number }[];
}

function VpsWorldMap({ countryData }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [tooltip, setTooltip] = useState<{
    content: string;
    x: number;
    y: number;
  } | null>(null);

  // Build lookup: alpha2 → count
  const countMap: Record<string, number> = {};
  for (const { name, value } of countryData) {
    countMap[name] = value;
  }

  const maxCount = Math.max(...countryData.map((d) => d.value), 1);

  const getColor = (alpha2: string): string => {
    const count = countMap[alpha2] || 0;
    if (count === 0) return '#f3f4f6';
    // Square root scale for better visual spread
    const t = Math.sqrt(count / maxCount);
    // Interpolate: #dbeafe (blue-100) → #1e40af (blue-800)
    const r = Math.round(219 + t * (30 - 219));
    const g = Math.round(234 + t * (64 - 234));
    const b = Math.round(254 + t * (175 - 254));
    return `rgb(${r}, ${g}, ${b})`;
  };

  return (
    <div ref={containerRef} className="relative">
      <ComposableMap
        projection="geoNaturalEarth1"
        projectionConfig={{ scale: 155 }}
        height={420}
        style={{ width: '100%', height: 'auto' }}
      >
        <Geographies geography={GEO_URL}>
          {({ geographies }) =>
            geographies.map((geo) => {
              const alpha2 = NUM_TO_A2[geo.id] || '';
              const count = countMap[alpha2] || 0;
              return (
                <Geography
                  key={geo.rsmKey}
                  geography={geo}
                  fill={getColor(alpha2)}
                  stroke="#d1d5db"
                  strokeWidth={0.4}
                  style={{
                    default: { outline: 'none', transition: 'fill 0.15s' },
                    hover: {
                      outline: 'none',
                      fill: count > 0 ? '#2563eb' : '#e5e7eb',
                      stroke: '#9ca3af',
                      strokeWidth: 0.8,
                    },
                    pressed: { outline: 'none' },
                  }}
                  onMouseMove={(e) => {
                    if (!containerRef.current || !alpha2) return;
                    const rect = containerRef.current.getBoundingClientRect();
                    const name = getCountryName(alpha2);
                    setTooltip({
                      content:
                        count > 0
                          ? `${name} (${alpha2}): ${count} VPS`
                          : `${name} (${alpha2})`,
                      x: e.clientX - rect.left + 14,
                      y: e.clientY - rect.top - 8,
                    });
                  }}
                  onMouseLeave={() => setTooltip(null)}
                />
              );
            })
          }
        </Geographies>
      </ComposableMap>

      {/* Tooltip */}
      {tooltip && (
        <div
          className="absolute pointer-events-none bg-gray-900 text-white text-xs font-medium px-2.5 py-1.5 rounded shadow-lg whitespace-nowrap z-10"
          style={{ left: tooltip.x, top: tooltip.y }}
        >
          {tooltip.content}
        </div>
      )}

      {/* Legend */}
      <div className="absolute bottom-3 left-4 flex items-center gap-1.5 text-[11px] text-gray-500">
        <span>0</span>
        <div
          className="h-2 w-20 rounded-sm"
          style={{
            background: 'linear-gradient(to right, #dbeafe, #60a5fa, #1e40af)',
          }}
        />
        <span>{maxCount}+</span>
      </div>
    </div>
  );
}

export default memo(VpsWorldMap);
