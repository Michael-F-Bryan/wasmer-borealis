package wasmer_borealis

import (
	"encoding/json"
	"errors"
	"fmt"

	"gorm.io/gorm"
)

type RunningExperiment struct {
	gorm.Model
	// The JSON definition for this experiment.
	Definition string
}

func (e RunningExperiment) Experiment() (Experiment, error) {
	if e.Definition == "" {
		return Experiment{}, errors.New("the experiment definition was empty")
	}

	var experiment Experiment
	if err := json.Unmarshal([]byte(e.Definition), &experiment); err != nil {
		return Experiment{}, fmt.Errorf("invalid experiment definition: %w", err)
	}

	return experiment, nil
}

type TestCase struct {
	gorm.Model
}

func AutoMigrate(db *gorm.DB) error {
	return db.AutoMigrate(&RunningExperiment{}, &TestCase{})
}
