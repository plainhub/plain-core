import com.google.i18n.phonenumbers.NumberParseException;
import com.google.i18n.phonenumbers.PhoneNumberUtil;
import com.google.i18n.phonenumbers.Phonenumber.PhoneNumber;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.stream.Collectors;

public final class PhoneNumberReference {
    private static final PhoneNumberUtil UTIL = PhoneNumberUtil.getInstance();

    private static String resolve(String input, String region) {
        try {
            // Canonicalize decimal code points before the Java parser's UTF-16 character normalization.
            StringBuilder normalized = new StringBuilder(input.length());
            input.codePoints().forEach(point -> {
                int digit = Character.digit(point, 10);
                normalized.appendCodePoint(digit >= 0 ? '0' + digit : point);
            });
            PhoneNumber number = UTIL.parse(normalized.toString(), region);
            if (!UTIL.isValidNumber(number)) return "null";
            String validRegions = UTIL.getRegionCodesForCountryCode(number.getCountryCode()).stream()
                    .filter(candidate -> UTIL.isValidNumberForRegion(number, candidate)).sorted()
                    .map(candidate -> "\"" + candidate + "\"").collect(Collectors.joining(","));
            return "{\"country_code\":" + number.getCountryCode()
                    + ",\"national\":\"" + UTIL.getNationalSignificantNumber(number)
                    + "\",\"kind\":\"" + UTIL.getNumberType(number).name()
                    + "\",\"main_region\":\"" + UTIL.getRegionCodeForCountryCode(number.getCountryCode())
                    + "\",\"valid_regions\":[" + validRegions + "]}";
        } catch (NumberParseException exception) {
            return "null";
        }
    }

    public static void main(String[] args) throws Exception {
        try (BufferedReader input = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8))) {
            for (String line; (line = input.readLine()) != null;) {
                String[] fields = line.split("\t", 2);
                String number = new String(Base64.getDecoder().decode(fields[1]), StandardCharsets.UTF_8);
                System.out.println(resolve(number, fields[0]));
            }
        }
    }
}
