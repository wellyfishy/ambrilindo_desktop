# Seed for the desktop <-> web end-to-end tests (src-tauri/src/web_e2e.rs).
# Runs as `manage.py shell -c <this file>` against a throw-away SQLite database made by the
# test harness; it never touches a real web database. Prints one line: E2E_SEED=<json>.
import datetime
import json

from django.contrib.auth.models import User

from backend.licensing import generate_license_key
from backend.models import Event, EventSyncGrant, License, LicenseActivation, UserType
from backend.views.license_api import _license_response

DEVICE = "d" * 64


def make_license(name, **extra):
    return License.objects.create(key=generate_license_key(), buyer_name=name, max_devices=2, **extra)


def token_for(lic, device=DEVICE):
    act, _ = LicenseActivation.objects.get_or_create(license=lic, device_id=device, defaults={"is_active": True})
    act.is_active = True
    act.save()
    return json.loads(_license_response(lic, act).content)["token"], act.pk


event_a = Event.objects.create(nama_event="E2E Event A", realtime=True)
event_b = Event.objects.create(nama_event="E2E Event B", realtime=True)

main = make_license("E2E main")
EventSyncGrant.objects.create(event=event_a, license=main)
main_token, _ = token_for(main)

no_grant = make_license("E2E no grant")
no_grant_token, _ = token_for(no_grant)

revoked = make_license("E2E revoked")
EventSyncGrant.objects.create(event=event_a, license=revoked)
revoked_token, _ = token_for(revoked)
revoked.is_revoked = True
revoked.save()

expired = make_license("E2E expired", expires_at=datetime.date.today() - datetime.timedelta(days=3))
EventSyncGrant.objects.create(event=event_a, license=expired)
expired_token, _ = token_for(expired)

released = make_license("E2E released")
EventSyncGrant.objects.create(event=event_a, license=released)
released_token, released_act = token_for(released)
LicenseActivation.objects.filter(pk=released_act).update(is_active=False)

admin = User.objects.create_user("e2e-admin", "e2e-admin@example.test", "e2e-pass-123")
UserType.objects.create(user=admin, is_admin=True)

print("E2E_SEED=" + json.dumps({
    "event_a": event_a.pk,
    "event_b": event_b.pk,
    "device": DEVICE,
    "main": {"pk": main.pk, "token": main_token},
    "no_grant": {"pk": no_grant.pk, "token": no_grant_token},
    "revoked": {"pk": revoked.pk, "token": revoked_token},
    "expired": {"pk": expired.pk, "token": expired_token},
    "released": {"pk": released.pk, "token": released_token},
    "admin": {"username": "e2e-admin", "password": "e2e-pass-123"},
}))
