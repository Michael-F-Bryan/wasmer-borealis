package wasmer_borealis

import (
	"database/sql/driver"
	"encoding/json"
	"errors"
	"fmt"

	"gorm.io/gorm"
)

func AutoMigrate(db *gorm.DB) error {
	return db.AutoMigrate(
		&Experiment{}, &TestCase{}, &Registry{}, &Owner{}, &Package{},
		&PackageVersion{}, &Blob{},
	)
}

type Experiment struct {
	gorm.Model
	// The JSON definition for this experiment.
	Definition string     `json:"definition"`
	TestCases  []TestCase `gorm:"constraint:OnDelete:CASCADE"`
}

func (e Experiment) Config() (Config, error) {
	if e.Definition == "" {
		return Config{}, errors.New("the experiment definition was empty")
	}

	var experiment Config
	if err := json.Unmarshal([]byte(e.Definition), &experiment); err != nil {
		return Config{}, fmt.Errorf("invalid experiment definition: %w", err)
	}

	return experiment, nil
}

type TestCase struct {
	gorm.Model
	PackageVersionID uint
	PackageVersion   PackageVersion
	ExperimentID     uint
	State            TestCaseState
	Outcome          *Outcome `gorm:"constraint:OnDelete:CASCADE"`
}

type TestCaseState int

const (
	TestCaseStateQueued    = iota
	TestCaseStateRunning   = iota
	TestCaseStateErrored   = iota
	TestCaseStateSucceeded = iota
)

func (t TestCaseState) String() string {
	switch t {
	case TestCaseStateQueued:
		return "queued"
	case TestCaseStateRunning:
		return "running"
	case TestCaseStateErrored:
		return "errored"
	case TestCaseStateSucceeded:
		return "succeeded"
	default:
		return fmt.Sprintf("<invalid: %d>", t)
	}
}

func (t *TestCaseState) Scan(value interface{}) error {
	raw, ok := value.(int)
	if !ok {
		return fmt.Errorf("expected an integer, found %v", value)
	}

	*t = TestCaseState(raw)
	return nil
}

func (t TestCaseState) Value() (driver.Value, error) {
	return driver.DefaultParameterConverter.ConvertValue(int(t))
}

func (t TestCaseState) MarshalJSON() ([]byte, error) {
	return []byte(t.String()), nil
}

func (t *TestCaseState) UnmarshalJSON(data []byte) error {
	var s string
	if err := json.Unmarshal(data, &s); err != nil {
		return err
	}

	switch s {
	case "queued":
		*t = TestCaseStateQueued
	case "running":
		*t = TestCaseStateRunning
	case "errored":
		*t = TestCaseStateErrored
	case "succeeded":
		*t = TestCaseStateSucceeded
	default:
		return fmt.Errorf("unknown test case state: %s", s)
	}

	return nil
}

type Outcome struct {
	gorm.Model
	TestCaseID uint
	ExitCode   int
	Stdout     string
	Stderr     string
}

type Registry struct {
	gorm.Model
	Endpoint        string `gorm:"unique"`
	Token           string
	Owners          []Owner          `gorm:"constraint:OnDelete:CASCADE"`
	Packages        []Package        `gorm:"constraint:OnDelete:CASCADE"`
	PackageVersions []PackageVersion `gorm:"constraint:OnDelete:CASCADE"`
}

type Owner struct {
	gorm.Model
	RegistryID uint   `gorm:"index:idx_owner,unique"`
	Name       string `gorm:"index:idx_owner,unique"`
	OwnerType  OwnerType
	Packages   []Package `gorm:"constraint:OnDelete:CASCADE"`
}

type Package struct {
	gorm.Model
	RegistryID uint
	OwnerID    uint             `gorm:"index:idx_package,unique"`
	Name       string           `gorm:"index:idx_package,unique"`
	Versions   []PackageVersion `gorm:"constraint:OnDelete:CASCADE"`
}

type PackageVersion struct {
	gorm.Model
	PackageID  uint
	RegistryID uint
	OwnerID    uint
	Version    string
	UpstreamID string
	WebcId     uint
	Webc       *Blob
	TarballId  uint
	Tarball    *Blob
}

type Blob struct {
	gorm.Model
	Sha256 string `gorm:"uniqueIndex"`
	Bytes  []byte
}

type OwnerType int64

const (
	OwnerUser      OwnerType = iota
	OwnerNamespace OwnerType = iota
	ownerUnknown   OwnerType = iota
)

func (t OwnerType) String() string {
	switch t {
	case OwnerUser:
		return "user"
	case OwnerNamespace:
		return "namespace"
	default:
		return fmt.Sprintf("<invalid: %d>", t)
	}
}

func (t *OwnerType) Scan(value interface{}) error {
	raw, ok := value.(int64)
	if !ok {
		return fmt.Errorf("expected an integer, found %v (%t)", value, value)
	}

	*t = OwnerType(raw)
	return nil
}

func (t OwnerType) Value() (driver.Value, error) {
	return driver.DefaultParameterConverter.ConvertValue(int(t))
}
